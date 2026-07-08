//! Build-cache warmup: run the probe once per cache dir before mutants, so
//! per-mutant probe runs start from a hot cache.

use super::template::{self, ProbeEnvCtx, ProbeEnvTemplate};
use crate::probe::ProbeCommand;
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn warmup(
    workspace_path: &Path,
    probe: &ProbeCommand,
    build_cache_dir: Option<&Path>,
    extra_envs: &[(String, String)],
) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new(probe.program());
    cmd.args(probe.args()).current_dir(workspace_path);

    if let Some(dir) = build_cache_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating build cache dir {}", dir.display()))?;
    }

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    cmd.status()
        .with_context(|| format!("running warmup command {probe:?}"))
}

/// `workspaces[i]` is the directory worker `i` warms up in (the repo root, or
/// that worker's worktree); indices past the slice clamp to the last entry.
pub fn warmup_workers(
    workspaces: &[PathBuf],
    probe: &ProbeCommand,
    target_dirs: &[PathBuf],
    jobs: usize,
    probe_env_templates: &[ProbeEnvTemplate],
) -> Result<()> {
    if target_dirs.is_empty() || workspaces.is_empty() {
        return Ok(());
    }

    let run_one = |idx: usize, dir: &PathBuf| -> Result<()> {
        let extra_envs = template::eval_all(
            probe_env_templates,
            ProbeEnvCtx {
                worker: idx,
                build_cache: Some(dir),
            },
        );
        let workspace = &workspaces[idx.min(workspaces.len() - 1)];
        let status = warmup(workspace, probe, Some(dir), &extra_envs)?;
        if !status.success() {
            bail!("warmup failed in {} with status {status}", dir.display());
        }
        Ok(())
    };

    // Copy/overlay backends warm multiple cache dirs from the same checkout. Do
    // not run ordinary test suites concurrently in one source tree: tests may
    // share fixture paths or repo-local state even when build caches differ.
    if workspaces_share_source(workspaces, target_dirs.len()) {
        for (idx, dir) in target_dirs.iter().enumerate() {
            run_one(idx, dir)?;
        }
        return Ok(());
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs.max(1))
        .build()
        .context("building warmup pool")?;
    pool.install(|| {
        target_dirs
            .par_iter()
            .enumerate()
            .try_for_each(|(idx, dir)| run_one(idx, dir))
    })
}

fn workspaces_share_source(workspaces: &[PathBuf], target_count: usize) -> bool {
    (0..target_count).any(|idx| {
        let workspace = &workspaces[idx.min(workspaces.len() - 1)];
        (0..idx).any(|seen_idx| {
            let seen = &workspaces[seen_idx.min(workspaces.len() - 1)];
            seen == workspace
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shared_warmup_workspace() {
        assert!(workspaces_share_source(
            &[PathBuf::from("/repo"), PathBuf::from("/repo")],
            2
        ));
        assert!(
            workspaces_share_source(&[PathBuf::from("/repo")], 2),
            "clamped workspace indexes share the same source"
        );
        assert!(!workspaces_share_source(
            &[
                PathBuf::from("/repo/worktree-0"),
                PathBuf::from("/repo/worktree-1"),
            ],
            2
        ));
    }
}
