//! Batch drivers: run a set of mutation candidates sequentially or across a
//! rayon worker pool, materializing each mutant in a workspace and probing it.

use super::process::run_probe;
use super::template::{self, ProbeEnvCtx, ProbeEnvTemplate};
use crate::core::{MutantOutcome, MutantStatus, MutationCandidate, MutationRunReport};
use crate::probe::ProbeCommand;
use crate::workspace::{self, WorkspaceBackend, worktree};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Clone)]
pub struct BatchConfig<'a> {
    pub backend: WorkspaceBackend,
    pub timeout: Option<Duration>,
    pub build_cache_dir: Option<&'a Path>,
    pub worker_build_cache_dirs: Option<&'a [PathBuf]>,
    pub probe_env_templates: &'a [ProbeEnvTemplate],
    pub runs_dir: &'a Path,
    pub progress: Option<fn(ProgressEvent<'_>)>,
    /// Per-worker worktrees, required when `backend` is `Worktree`. Unlike the
    /// other backends these are created once up front and reused across
    /// mutants, so the pool lives outside the per-mutant run.
    pub worktree_pool: Option<&'a worktree::WorktreePool>,
}

pub struct ProgressEvent<'a> {
    pub completed: usize,
    pub total: usize,
    pub outcome: &'a MutantOutcome,
}

fn run_one(
    repo_root: &Path,
    candidate: MutationCandidate,
    probe: &ProbeCommand,
    cfg: &BatchConfig<'_>,
    run_id: &str,
) -> MutantOutcome {
    match try_run_one(repo_root, &candidate, probe, cfg, run_id) {
        Ok(outcome) => outcome,
        Err(err) => MutantOutcome {
            candidate,
            status: MutantStatus::Error,
            exit_code: None,
            duration_ms: 0,
            diff: String::new(),
            stdout: String::new(),
            stderr: format!("{err:#}"),
        },
    }
}

fn try_run_one(
    repo_root: &Path,
    candidate: &MutationCandidate,
    probe: &ProbeCommand,
    cfg: &BatchConfig<'_>,
    run_id: &str,
) -> Result<MutantOutcome> {
    let worker_idx = rayon::current_thread_index().unwrap_or(0);

    // The worktree backend reuses one worktree per worker; the others build a
    // fresh workspace per mutant. Keep the per-mutant workspace alive until
    // the probe finishes.
    let per_mutant_workspace;
    let pooled_worktree;
    let workspace_path: &Path = if cfg.backend == WorkspaceBackend::Worktree {
        let pool = cfg
            .worktree_pool
            .context("worktree backend selected but no worktree pool was created")?;
        pool.reset(worker_idx)
            .with_context(|| format!("resetting worktree before {}", candidate.id))?;
        pooled_worktree = pool.path_for(worker_idx);
        &pooled_worktree
    } else {
        per_mutant_workspace =
            workspace::create_workspace(cfg.backend, repo_root, cfg.runs_dir, run_id)
                .with_context(|| format!("creating workspace for {}", candidate.id))?;
        per_mutant_workspace.path()
    };

    let applied = workspace::apply_mutation(workspace_path, repo_root, candidate)
        .with_context(|| format!("applying mutation {}", candidate.id))?;
    let worker_build_cache: Option<PathBuf> = cfg.worker_build_cache_dirs.and_then(|dirs| {
        dirs.get(worker_idx)
            .cloned()
            .or_else(|| dirs.first().cloned())
    });
    let build_cache_dir = worker_build_cache.as_deref().or(cfg.build_cache_dir);

    let extra_envs = template::eval_all(
        cfg.probe_env_templates,
        ProbeEnvCtx {
            worker: worker_idx,
            build_cache: build_cache_dir,
        },
    );

    let outcome = run_probe(workspace_path, applied, probe, cfg.timeout, &extra_envs)
        .with_context(|| format!("running probe for {}", candidate.id))?;

    // Leave the reused worktree clean for the next mutant; best-effort since
    // the next mutant resets again before applying.
    if cfg.backend == WorkspaceBackend::Worktree
        && let Some(pool) = cfg.worktree_pool
    {
        let _ = pool.reset(worker_idx);
    }

    Ok(outcome)
}

fn run_id_for(idx: usize, candidate_id: &str) -> String {
    let safe: String = candidate_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("mutant-{idx:05}-{safe}")
}

pub fn run_mutants_sequential(
    repo_root: &Path,
    candidates: Vec<MutationCandidate>,
    probe: &ProbeCommand,
    cfg: &BatchConfig<'_>,
) -> MutationRunReport {
    let total = candidates.len();
    let outcomes: Vec<MutantOutcome> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, candidate)| {
            let id = run_id_for(i, &candidate.id);
            let outcome = run_one(repo_root, candidate, probe, cfg, &id);
            if let Some(cb) = cfg.progress {
                cb(ProgressEvent {
                    completed: i + 1,
                    total,
                    outcome: &outcome,
                });
            }
            outcome
        })
        .collect();

    MutationRunReport::from_outcomes(outcomes)
}

pub fn run_mutants_parallel(
    repo_root: &Path,
    candidates: Vec<MutationCandidate>,
    probe: &ProbeCommand,
    jobs: usize,
    cfg: &BatchConfig<'_>,
) -> Result<MutationRunReport> {
    if jobs <= 1 {
        return Ok(run_mutants_sequential(repo_root, candidates, probe, cfg));
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .context("building mutation worker pool")?;

    let total = candidates.len();
    let indexed: Vec<(usize, MutationCandidate)> = candidates.into_iter().enumerate().collect();
    let completed = std::sync::atomic::AtomicUsize::new(0);

    let outcomes = pool.install(|| {
        indexed
            .into_par_iter()
            .map(|(i, candidate)| {
                let id = run_id_for(i, &candidate.id);
                let outcome = run_one(repo_root, candidate, probe, cfg, &id);
                if let Some(cb) = cfg.progress {
                    let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                    cb(ProgressEvent {
                        completed: done,
                        total,
                        outcome: &outcome,
                    });
                }
                outcome
            })
            .collect()
    });

    Ok(MutationRunReport::from_outcomes(outcomes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_sanitizes_candidate_ids() {
        assert_eq!(
            run_id_for(7, "src/lib.rs:3 a<b"),
            "mutant-00007-src_lib_rs_3_a_b"
        );
    }
}
