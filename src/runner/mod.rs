use crate::core::{
    AppliedMutation, MutantOutcome, MutantStatus, MutationCandidate, MutationRunReport,
};
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use similar::{ChangeTag, TextDiff};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use walkdir::WalkDir;

pub mod overlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceBackend {
    Copy,
    Overlay,
}

pub enum Workspace {
    Copy(CowWorkspace),
    Overlay(overlay::OverlayWorkspace),
}

impl Workspace {
    pub fn path(&self) -> &Path {
        match self {
            Workspace::Copy(w) => w.path(),
            Workspace::Overlay(w) => w.path(),
        }
    }
}

pub struct CowWorkspace {
    root: TempDir,
}

impl CowWorkspace {
    pub fn create_from_repo(repo_root: &Path) -> Result<Self> {
        let root = tempfile::tempdir().context("creating temp workspace")?;
        copy_repo(repo_root, root.path())?;
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn apply_mutation(
        &self,
        repo_root: &Path,
        candidate: &MutationCandidate,
    ) -> Result<AppliedMutation> {
        apply_mutation(self.path(), repo_root, candidate)
    }

    pub fn run_probe(
        &self,
        applied: AppliedMutation,
        probe: &[String],
        timeout: Option<Duration>,
    ) -> Result<MutantOutcome> {
        run_probe(self.path(), applied, probe, timeout, &[])
    }
}

pub fn apply_mutation(
    workspace_path: &Path,
    repo_root: &Path,
    candidate: &MutationCandidate,
) -> Result<AppliedMutation> {
    let relative_file = candidate
        .file
        .strip_prefix(repo_root)
        .unwrap_or(&candidate.file);

    let workspace_file = workspace_path.join(relative_file);

    let original = std::fs::read_to_string(&workspace_file)
        .with_context(|| format!("reading workspace file {}", workspace_file.display()))?;

    let start = candidate.start_byte;
    let end = candidate.end_byte;

    if start > end || end > original.len() {
        bail!(
            "candidate byte range {}..{} is invalid for {}",
            start,
            end,
            workspace_file.display()
        );
    }

    let found = &original[start..end];
    if found != candidate.original {
        bail!(
            "candidate original text mismatch in {}: expected {:?}, found {:?}",
            workspace_file.display(),
            candidate.original,
            found
        );
    }

    let mut mutated =
        String::with_capacity(original.len() - (end - start) + candidate.replacement.len());
    mutated.push_str(&original[..start]);
    mutated.push_str(&candidate.replacement);
    mutated.push_str(&original[end..]);

    std::fs::write(&workspace_file, &mutated)
        .with_context(|| format!("writing workspace file {}", workspace_file.display()))?;

    let diff = unified_diff(&relative_file.to_string_lossy(), &original, &mutated);

    Ok(AppliedMutation {
        candidate: candidate.clone(),
        workspace_file,
        diff,
    })
}

pub fn run_probe(
    workspace_path: &Path,
    applied: AppliedMutation,
    probe: &[String],
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<MutantOutcome> {
    if probe.is_empty() {
        bail!("probe command is empty");
    }

    let started = Instant::now();

    let mut cmd = Command::new(&probe[0]);
    cmd.args(&probe[1..])
        .current_dir(workspace_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning probe command {:?}", probe))?;

    let mut timed_out = false;

    let status = loop {
        match child.try_wait().context("polling probe child")? {
            Some(s) => break Some(s),
            None => {
                if let Some(limit) = timeout
                    && started.elapsed() >= limit
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let duration_ms = started.elapsed().as_millis();

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut stderr);
    }

    let (mutant_status, exit_code) = if timed_out {
        (MutantStatus::Timeout, None)
    } else {
        let s = status.expect("status set when not timed out");
        let code = s.code();
        let mutant_status = if s.success() {
            MutantStatus::Survived
        } else {
            MutantStatus::Killed
        };
        (mutant_status, code)
    };

    Ok(MutantOutcome {
        candidate: applied.candidate,
        status: mutant_status,
        exit_code,
        duration_ms,
        diff: applied.diff,
        stdout,
        stderr,
    })
}

#[derive(Clone)]
pub struct BatchConfig<'a> {
    pub backend: WorkspaceBackend,
    pub timeout: Option<Duration>,
    pub build_cache_dir: Option<&'a Path>,
    pub worker_build_cache_dirs: Option<&'a [PathBuf]>,
    pub probe_env_templates: &'a [(String, String)],
    pub runs_dir: &'a Path,
    pub progress: Option<fn(ProgressEvent<'_>)>,
}

pub struct ProgressEvent<'a> {
    pub completed: usize,
    pub total: usize,
    pub outcome: &'a MutantOutcome,
}

fn create_workspace(
    backend: WorkspaceBackend,
    repo_root: &Path,
    runs_dir: &Path,
    run_id: &str,
) -> Result<Workspace> {
    match backend {
        WorkspaceBackend::Copy => {
            CowWorkspace::create_from_repo(repo_root).map(Workspace::Copy)
        }
        WorkspaceBackend::Overlay => {
            overlay::OverlayWorkspace::create(repo_root, runs_dir, run_id).map(Workspace::Overlay)
        }
    }
}

fn run_one(
    repo_root: &Path,
    candidate: MutationCandidate,
    probe: &[String],
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
    probe: &[String],
    cfg: &BatchConfig<'_>,
    run_id: &str,
) -> Result<MutantOutcome> {
    let workspace = create_workspace(cfg.backend, repo_root, cfg.runs_dir, run_id)
        .with_context(|| format!("creating workspace for {}", candidate.id))?;

    let applied = apply_mutation(workspace.path(), repo_root, candidate)
        .with_context(|| format!("applying mutation {}", candidate.id))?;

    let worker_idx = rayon::current_thread_index().unwrap_or(0);
    let worker_build_cache: Option<PathBuf> = cfg.worker_build_cache_dirs.and_then(|dirs| {
        dirs.get(worker_idx).cloned().or_else(|| dirs.first().cloned())
    });
    let build_cache_dir = worker_build_cache.as_deref().or(cfg.build_cache_dir);

    let extra_envs: Vec<(String, String)> = cfg
        .probe_env_templates
        .iter()
        .map(|(k, v)| {
            let v = v.replace("{worker}", &worker_idx.to_string());
            let v = if let Some(dir) = build_cache_dir {
                v.replace("{build_cache}", &dir.to_string_lossy())
            } else {
                v
            };
            (k.clone(), v)
        })
        .collect();

    run_probe(
        workspace.path(),
        applied,
        probe,
        cfg.timeout,
        &extra_envs,
    )
    .with_context(|| format!("running probe for {}", candidate.id))
}

fn run_id_for(idx: usize, candidate: &MutationCandidate) -> String {
    let safe: String = candidate
        .id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("mutant-{:05}-{}", idx, safe)
}

pub fn run_mutants_sequential(
    repo_root: &Path,
    candidates: Vec<MutationCandidate>,
    probe: &[String],
    cfg: BatchConfig<'_>,
) -> Result<MutationRunReport> {
    let total = candidates.len();
    let outcomes: Vec<MutantOutcome> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, candidate)| {
            let id = run_id_for(i, &candidate);
            let outcome = run_one(repo_root, candidate, probe, &cfg, &id);
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

    Ok(MutationRunReport::from_outcomes(outcomes))
}

pub fn run_mutants_parallel(
    repo_root: &Path,
    candidates: Vec<MutationCandidate>,
    probe: &[String],
    jobs: usize,
    cfg: BatchConfig<'_>,
) -> Result<MutationRunReport> {
    if jobs <= 1 {
        return run_mutants_sequential(repo_root, candidates, probe, cfg);
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
                let id = run_id_for(i, &candidate);
                let outcome = run_one(repo_root, candidate, probe, &cfg, &id);
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreflightOutcome {
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub stdout: String,
    pub stderr: String,
}

pub fn preflight(
    repo_root: &Path,
    probe: &[String],
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<PreflightOutcome> {
    if probe.is_empty() {
        bail!("preflight probe command is empty");
    }

    let started = Instant::now();

    let mut cmd = Command::new(&probe[0]);
    cmd.args(&probe[1..])
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning preflight probe {:?}", probe))?;

    let mut timed_out = false;
    let status = loop {
        match child.try_wait().context("polling preflight child")? {
            Some(s) => break Some(s),
            None => {
                if let Some(limit) = timeout
                    && started.elapsed() >= limit
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let duration_ms = started.elapsed().as_millis();

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut stderr);
    }

    let (success, exit_code) = if timed_out {
        (false, None)
    } else {
        let s = status.expect("status set when not timed out");
        (s.success(), s.code())
    };

    Ok(PreflightOutcome {
        success,
        timed_out,
        exit_code,
        duration_ms,
        stdout,
        stderr,
    })
}

pub fn warmup(
    workspace_path: &Path,
    probe: &[String],
    build_cache_dir: Option<&Path>,
    extra_envs: &[(String, String)],
) -> Result<std::process::ExitStatus> {
    if probe.is_empty() {
        bail!("warmup command is empty");
    }

    let mut cmd = Command::new(&probe[0]);
    cmd.args(&probe[1..]).current_dir(workspace_path);

    if let Some(dir) = build_cache_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating build cache dir {}", dir.display()))?;
    }

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    cmd.status()
        .with_context(|| format!("running warmup command {:?}", probe))
}

pub fn default_build_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("build-cache")
}

pub fn warmup_workers(
    workspace_path: &Path,
    probe: &[String],
    target_dirs: &[PathBuf],
    jobs: usize,
    probe_env_templates: &[(String, String)],
) -> Result<()> {
    if target_dirs.is_empty() {
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
            .try_for_each(|(idx, dir)| -> Result<()> {
                let extra_envs: Vec<(String, String)> = probe_env_templates
                    .iter()
                    .map(|(k, v)| {
                        let v = v.replace("{worker}", &idx.to_string());
                        let v = v.replace("{build_cache}", &dir.to_string_lossy());
                        (k.clone(), v)
                    })
                    .collect();
                let status = warmup(workspace_path, probe, Some(dir), &extra_envs)?;
                if !status.success() {
                    bail!("warmup failed in {} with status {status}", dir.display());
                }
                Ok(())
            })
    })
}

fn copy_repo(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();

        let relative = path
            .strip_prefix(src)
            .with_context(|| format!("stripping repo prefix from {}", path.display()))?;

        if should_skip(relative) {
            if entry.file_type().is_dir() {
                continue;
            }
            continue;
        }

        let target = dst.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("creating dir {}", target.display()))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent {}", parent.display()))?;
            }

            std::fs::copy(path, &target)
                .with_context(|| format!("copying {} -> {}", path.display(), target.display()))?;
        }
    }

    Ok(())
}

fn should_skip(relative: &Path) -> bool {
    let first = relative.components().next();

    let Some(first) = first else {
        return false;
    };

    let first = first.as_os_str().to_string_lossy();

    matches!(
        first.as_ref(),
        ".git"
            | ".ooze"
            | "target"
            | "node_modules"
            | "vendor"
            | "__pycache__"
            | ".gradle"
            | ".direnv"
            | ".idea"
            | ".vscode"
    )
}

fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);

    let mut output = String::new();
    output.push_str(&format!("--- a/{path}\n"));
    output.push_str(&format!("+++ b/{path}\n"));

    for group in diff.grouped_ops(3) {
        for op in group {
            for change in diff.iter_changes(&op) {
                match change.tag() {
                    ChangeTag::Delete => output.push('-'),
                    ChangeTag::Insert => output.push('+'),
                    ChangeTag::Equal => output.push(' '),
                }

                output.push_str(change.value());
            }
        }
    }

    output
}
