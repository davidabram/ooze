//! Git worktree workspace backend: one detached worktree per worker, reused
//! across mutants. Rootless and CI-friendly, but requires running inside a
//! Git repository; mutants are applied against the content of `HEAD`.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directory (under the runs dir) that holds all ooze-managed worktrees.
/// Only paths inside it are ever removed destructively.
const WORKTREES_SUBDIR: &str = "worktrees";

/// Prefix for per-worker worktree directory names (`wt-0`, `wt-1`, ...).
const WORKTREE_PREFIX: &str = "wt-";

pub fn is_git_repo(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn git(repo: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .with_context(|| format!("running git {args:?} (is git installed?)"))?;

    if !output.status.success() {
        bail!(
            "git {} failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[derive(Debug)]
pub struct WorktreePool {
    repo_root: PathBuf,
    worktrees_dir: PathBuf,
    paths: Vec<PathBuf>,
}

impl WorktreePool {
    /// Create `workers` detached worktrees of the repo's `HEAD` under
    /// `<runs_dir>/worktrees/wt-{i}`. Stale worktrees left by a crashed run
    /// are removed automatically (they live under an ooze-managed path).
    pub fn create(repo_root: &Path, runs_dir: &Path, workers: usize) -> Result<Self> {
        if !is_git_repo(repo_root) {
            bail!(
                "the worktree workspace backend requires a Git repository, but {} is not inside one. \
                 Run `git init` (and commit your code) or choose --workspace-backend copy.",
                repo_root.display()
            );
        }

        let repo_root = repo_root
            .canonicalize()
            .with_context(|| format!("canonicalizing {}", repo_root.display()))?;
        let worktrees_dir = runs_dir.join(WORKTREES_SUBDIR);

        remove_stale_worktrees(&repo_root, &worktrees_dir)?;

        std::fs::create_dir_all(&worktrees_dir)
            .with_context(|| format!("creating worktrees dir {}", worktrees_dir.display()))?;

        let mut paths = Vec::with_capacity(workers.max(1));
        for i in 0..workers.max(1) {
            let path = worktrees_dir.join(format!("{WORKTREE_PREFIX}{i}"));
            git(
                &repo_root,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    &path.to_string_lossy(),
                    "HEAD",
                ],
            )
            .with_context(|| format!("creating git worktree {}", path.display()))?;
            paths.push(path);
        }

        Ok(Self {
            repo_root,
            worktrees_dir,
            paths,
        })
    }

    /// Worktree path for a worker index. Indices beyond the pool clamp to the
    /// first worktree (mirrors how per-worker cache dirs are picked).
    pub fn path_for(&self, worker: usize) -> &Path {
        self.paths
            .get(worker)
            .unwrap_or_else(|| &self.paths[0])
            .as_path()
    }

    /// Reset a worker's worktree to a pristine `HEAD` checkout, discarding
    /// tracked modifications and untracked/ignored files (build artifacts,
    /// test output). Destructive, but only ever runs inside an ooze-managed
    /// worktree under `.ooze/runs/worktrees`.
    pub fn reset(&self, worker: usize) -> Result<()> {
        let wt = self.path_for(worker);
        git(wt, &["reset", "--hard", "HEAD"])
            .with_context(|| format!("resetting worktree {}", wt.display()))?;
        git(wt, &["clean", "-fdx"])
            .with_context(|| format!("cleaning worktree {}", wt.display()))?;
        Ok(())
    }

    /// Remove all worktrees and, if it ends up empty, the worktrees dir.
    pub fn cleanup(&mut self) -> Result<()> {
        let mut first_err: Option<anyhow::Error> = None;
        for path in self.paths.drain(..) {
            if let Err(e) = git(
                &self.repo_root,
                &["worktree", "remove", "--force", &path.to_string_lossy()],
            ) {
                first_err.get_or_insert(e);
            }
        }
        remove_dir_if_empty(&self.worktrees_dir);
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl Drop for WorktreePool {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Remove leftover `wt-*` worktrees from a previous (crashed) run. Only
/// touches entries directly under the ooze-managed worktrees dir.
fn remove_stale_worktrees(repo_root: &Path, worktrees_dir: &Path) -> Result<()> {
    if !worktrees_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(worktrees_dir)
        .with_context(|| format!("reading {}", worktrees_dir.display()))?
    {
        let path = entry?.path();
        let is_ours = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(WORKTREE_PREFIX));
        if !is_ours {
            continue;
        }

        // Ask git first so its worktree bookkeeping stays consistent; fall
        // back to deleting the directory if git no longer knows about it.
        let _ = git(
            repo_root,
            &["worktree", "remove", "--force", &path.to_string_lossy()],
        );
        if path.exists() {
            std::fs::remove_dir_all(&path).with_context(|| {
                format!("removing stale ooze worktree {}", path.display())
            })?;
        }
    }

    // Drop metadata for any worktree directories we deleted by hand.
    let _ = git(repo_root, &["worktree", "prune"]);
    Ok(())
}

fn remove_dir_if_empty(dir: &Path) {
    if std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_git_repo(dir: &Path) {
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .status()
                .expect("running git");
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("file.txt"), "hello\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
    }

    #[test]
    fn errors_clearly_outside_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let runs = tmp.path().join("runs");
        let err = WorktreePool::create(tmp.path(), &runs, 1).unwrap_err();
        assert!(
            err.to_string().contains("Git repository"),
            "error should mention Git: {err}"
        );
    }

    #[test]
    fn creates_ooze_managed_worktrees_per_worker() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let runs = tmp.path().join(".ooze/runs");

        let pool = WorktreePool::create(tmp.path(), &runs, 2).unwrap();
        assert_eq!(pool.paths.len(), 2);
        for i in 0..2 {
            let wt = pool.path_for(i);
            assert!(wt.starts_with(runs.join("worktrees")), "{}", wt.display());
            assert!(wt.ends_with(format!("wt-{i}")), "{}", wt.display());
            assert!(wt.join("file.txt").exists(), "worktree has repo content");
        }
    }

    #[test]
    fn reset_discards_tracked_edits_and_untracked_files() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let runs = tmp.path().join(".ooze/runs");

        let pool = WorktreePool::create(tmp.path(), &runs, 1).unwrap();
        let wt = pool.path_for(0).to_path_buf();

        std::fs::write(wt.join("file.txt"), "mutated\n").unwrap();
        std::fs::create_dir_all(wt.join("target")).unwrap();
        std::fs::write(wt.join("target/artifact"), "junk").unwrap();

        pool.reset(0).unwrap();

        assert_eq!(std::fs::read_to_string(wt.join("file.txt")).unwrap(), "hello\n");
        assert!(!wt.join("target").exists(), "untracked dirs cleaned");
    }

    #[test]
    fn cleanup_removes_worktrees_and_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let runs = tmp.path().join(".ooze/runs");

        let mut pool = WorktreePool::create(tmp.path(), &runs, 2).unwrap();
        let dir = runs.join("worktrees");
        assert!(dir.exists());

        pool.cleanup().unwrap();
        assert!(!dir.exists(), "empty worktrees dir removed");
    }

    #[test]
    fn stale_ooze_worktrees_are_removed_on_next_run() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        let runs = tmp.path().join(".ooze/runs");

        // Simulate a crashed run: worktrees exist but cleanup never ran.
        let pool = WorktreePool::create(tmp.path(), &runs, 1).unwrap();
        let stale = pool.path_for(0).to_path_buf();
        std::mem::forget(pool);
        assert!(stale.exists());

        let pool = WorktreePool::create(tmp.path(), &runs, 1).unwrap();
        assert!(pool.path_for(0).exists(), "fresh worktree usable after stale cleanup");
    }

    #[test]
    fn is_git_repo_detects_repos() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_git_repo(tmp.path()));
        init_git_repo(tmp.path());
        assert!(is_git_repo(tmp.path()));
    }
}
