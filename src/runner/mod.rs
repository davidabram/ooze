use crate::core::{
    AppliedMutation, MutantOutcome, MutantStatus, MutationCandidate, MutationRunReport,
};
use crate::probe::ProbeCommand;
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use similar::{ChangeTag, TextDiff};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use walkdir::WalkDir;

pub mod overlay;
pub mod template;
pub mod worktree;

pub use template::{ProbeEnvCtx, ProbeEnvTemplate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceBackend {
    Copy,
    Overlay,
    Worktree,
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
        probe: &ProbeCommand,
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

/// Result of running a child process to completion (or timeout). `status` is
/// `None` exactly when `timed_out` is true.
struct ProcessRun {
    status: Option<ExitStatus>,
    timed_out: bool,
    duration_ms: u128,
    stdout: String,
    stderr: String,
}

/// Wait for a spawned child while concurrently draining its stdout/stderr.
///
/// The pipes are read on separate threads so a probe that writes more than a
/// pipe buffer's worth (~64KB on Linux) before exiting can't block on its own
/// write and be misclassified as a timeout. `started` is the instant the child
/// was spawned, used for the timeout budget and reported duration.
fn wait_with_drained_output(
    mut child: Child,
    timeout: Option<Duration>,
    started: Instant,
) -> Result<ProcessRun> {
    // Take the pipe handles and drain each on its own thread. Both threads see
    // EOF once the child exits (or is killed), so they always finish.
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = out.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = err.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    let mut timed_out = false;
    let status = loop {
        if let Some(s) = child.try_wait().context("polling probe child")? {
            break Some(s);
        }
        if let Some(limit) = timeout
            && started.elapsed() >= limit
        {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let duration_ms = started.elapsed().as_millis();
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();

    Ok(ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    })
}

pub fn run_probe(
    workspace_path: &Path,
    applied: AppliedMutation,
    probe: &ProbeCommand,
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<MutantOutcome> {
    let started = Instant::now();

    let mut cmd = Command::new(probe.program());
    cmd.args(probe.args())
        .current_dir(workspace_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawning probe command {probe:?}"))?;

    let ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    } = wait_with_drained_output(child, timeout, started)?;

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

fn create_workspace(
    backend: WorkspaceBackend,
    repo_root: &Path,
    runs_dir: &Path,
    run_id: &str,
) -> Result<Workspace> {
    match backend {
        WorkspaceBackend::Copy => CowWorkspace::create_from_repo(repo_root).map(Workspace::Copy),
        WorkspaceBackend::Overlay => {
            overlay::OverlayWorkspace::create(repo_root, runs_dir, run_id).map(Workspace::Overlay)
        }
        WorkspaceBackend::Worktree => {
            bail!("worktree workspaces are pooled per worker, not created per mutant")
        }
    }
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
        per_mutant_workspace = create_workspace(cfg.backend, repo_root, cfg.runs_dir, run_id)
            .with_context(|| format!("creating workspace for {}", candidate.id))?;
        per_mutant_workspace.path()
    };

    let applied = apply_mutation(workspace_path, repo_root, candidate)
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

fn run_id_for(idx: usize, candidate: &MutationCandidate) -> String {
    let safe: String = candidate
        .id
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
            let id = run_id_for(i, &candidate);
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
                let id = run_id_for(i, &candidate);
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
    probe: &ProbeCommand,
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<PreflightOutcome> {
    let started = Instant::now();

    let mut cmd = Command::new(probe.program());
    cmd.args(probe.args())
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawning preflight probe {probe:?}"))?;

    let ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    } = wait_with_drained_output(child, timeout, started)?;

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

pub fn default_build_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("build-cache")
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

fn copy_repo(src: &Path, dst: &Path) -> Result<()> {
    copy_repo_with(src, dst, reflink_file)
}

fn copy_repo_with(
    src: &Path,
    dst: &Path,
    reflink: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let mut reflinked: u64 = 0;
    let mut copied: u64 = 0;

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

            if copy_file_with(&reflink, path, &target)? {
                reflinked += 1;
            } else {
                copied += 1;
            }
        }
    }

    tracing::debug!(reflinked, copied, "copy backend: workspace populated");

    Ok(())
}

/// Copy one regular file, attempting a reflink / copy-on-write clone first.
/// Returns `true` if the file was reflinked, `false` if it fell back to a normal
/// copy. Any reflink error (unsupported filesystem, cross-device, ...) is a
/// fallback, never a failure; only the normal copy's error is surfaced.
fn copy_file_with(
    reflink: impl Fn(&Path, &Path) -> std::io::Result<()>,
    src: &Path,
    dst: &Path,
) -> Result<bool> {
    if reflink(src, dst).is_ok() {
        return Ok(true);
    }

    std::fs::copy(src, dst)
        .with_context(|| format!("copying {} -> {}", src.display(), dst.display()))?;
    Ok(false)
}

/// Clone `src` to `dst` with the Linux `FICLONE` ioctl. Fails with the raw OS
/// error (EXDEV, EOPNOTSUPP, ENOTTY, EINVAL, ...) when the filesystem doesn't
/// support reflinks; callers treat any error as "fall back to a normal copy".
#[cfg(target_os = "linux")]
fn reflink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_int, c_ulong};

    unsafe extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }

    // _IOW(0x94, 9, int) from linux/fs.h.
    const FICLONE: c_ulong = 0x4004_9409;

    let src_file = std::fs::File::open(src)?;
    let dst_file = std::fs::File::create(dst)?;

    let rc = unsafe { ioctl(dst_file.as_raw_fd(), FICLONE, src_file.as_raw_fd()) };
    let result = if rc == 0 {
        // FICLONE clones contents only; carry over the source permissions the
        // same way std::fs::copy does.
        src_file
            .metadata()
            .and_then(|m| std::fs::set_permissions(dst, m.permissions()))
    } else {
        Err(std::io::Error::last_os_error())
    };

    if result.is_err() {
        drop(dst_file);
        let _ = std::fs::remove_file(dst);
    }
    result
}

#[cfg(not(target_os = "linux"))]
fn reflink_file(_src: &Path, _dst: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
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
    use std::fmt::Write;
    let diff = TextDiff::from_lines(old, new);

    let mut output = String::new();
    let _ = write!(output, "--- a/{path}\n+++ b/{path}\n");

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe that writes far more than a pipe buffer (~64KB) before exiting
    /// must be drained concurrently — otherwise it blocks on its own write and
    /// is misclassified as a timeout. 200KB exceeds the buffer several times
    /// over, so this fails (as a timeout) if the output isn't drained.
    #[test]
    #[cfg(unix)]
    fn large_probe_output_is_drained_without_false_timeout() {
        let probe = ProbeCommand::from_static(&["sh", "-c", "yes | head -c 200000"]);
        let outcome = preflight(Path::new("."), &probe, Some(Duration::from_secs(30)), &[])
            .expect("preflight should run");

        assert!(
            !outcome.timed_out,
            "200KB of output was misread as a timeout"
        );
        assert!(outcome.success, "probe exited 0");
        assert_eq!(outcome.stdout.len(), 200_000, "full stdout captured");
    }

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

    fn unsupported_reflink(_src: &Path, _dst: &Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    /// Build a small fake repo with a nested source file, an ignored
    /// directory, and (on unix) an executable script.
    fn make_repo() -> TempDir {
        let repo = tempfile::tempdir().expect("temp repo");
        let root = repo.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn one() -> i32 { 1 }\n").unwrap();
        std::fs::write(root.join("README.md"), "readme\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/junk"), "junk").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(root.join("run.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        repo
    }

    #[test]
    fn copy_repo_works_without_reflink_support() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).expect("copy succeeds");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
        assert_eq!(
            std::fs::read_to_string(dst.path().join("README.md")).unwrap(),
            "readme\n"
        );
    }

    #[test]
    fn copy_repo_skips_ignored_directories() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();

        assert!(
            !dst.path().join("target").exists(),
            "target must be skipped"
        );
        assert!(!dst.path().join(".git").exists(), ".git must be skipped");
    }

    #[test]
    fn copy_repo_leaves_original_untouched() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();
        std::fs::write(dst.path().join("src/lib.rs"), "mutated").unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n",
            "mutating the workspace must not touch the source checkout"
        );
    }

    #[test]
    #[cfg(unix)]
    fn copy_repo_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();

        let mode = std::fs::metadata(dst.path().join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "executable bit must survive the copy");
    }

    #[test]
    fn reflink_failure_falls_back_to_normal_copy() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();
        let src = repo.path().join("src/lib.rs");
        let target = dst.path().join("lib.rs");

        // Simulate a filesystem where reflink starts (creating the file) but
        // then fails, like a failing FICLONE ioctl leaving a partial dest.
        let flaky = |_s: &Path, d: &Path| -> std::io::Result<()> {
            std::fs::write(d, "partial")?;
            std::fs::remove_file(d)?;
            Err(std::io::Error::from_raw_os_error(18)) // EXDEV
        };

        let reflinked = copy_file_with(flaky, &src, &target).expect("fallback copy succeeds");
        assert!(!reflinked, "must report a normal copy, not a reflink");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
    }

    #[test]
    fn successful_reflink_skips_normal_copy() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();
        let src = repo.path().join("src/lib.rs");
        let target = dst.path().join("lib.rs");

        let fake_clone =
            |s: &Path, d: &Path| -> std::io::Result<()> { std::fs::copy(s, d).map(|_| ()) };

        let reflinked = copy_file_with(fake_clone, &src, &target).expect("clone succeeds");
        assert!(reflinked, "must report the reflink path was taken");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
    }

    /// The real reflink attempt must never break the copy backend: whether
    /// the test filesystem supports FICLONE or not, the workspace comes out
    /// identical.
    #[test]
    fn copy_repo_with_real_reflink_attempt() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo(repo.path(), dst.path()).expect("copy succeeds");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
        assert!(!dst.path().join("target").exists());
    }
}
