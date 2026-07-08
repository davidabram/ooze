use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct OverlayWorkspace {
    root: PathBuf,
    merged: PathBuf,
    mounted: bool,
}

impl OverlayWorkspace {
    pub fn create(repo_root: &Path, runs_root: &Path, run_id: &str) -> Result<Self> {
        if !cfg!(target_os = "linux") {
            bail!("overlayfs workspace backend is only supported on Linux");
        }

        let repo_root = repo_root
            .canonicalize()
            .with_context(|| format!("canonicalizing {}", repo_root.display()))?;

        std::fs::create_dir_all(runs_root)
            .with_context(|| format!("creating runs dir {}", runs_root.display()))?;

        let root = runs_root.join(run_id);
        let upper = root.join("upper");
        let work = root.join("work");
        let merged = root.join("merged");

        std::fs::create_dir_all(&upper)?;
        std::fs::create_dir_all(&work)?;
        std::fs::create_dir_all(&merged)?;

        let opts = format!(
            "lowerdir={},upperdir={},workdir={}",
            repo_root.display(),
            upper.display(),
            work.display(),
        );

        let status = Command::new("mount")
            .args(["-t", "overlay", "overlay", "-o", &opts])
            .arg(&merged)
            .status()
            .context("running overlayfs mount (you may need root or CAP_SYS_ADMIN)")?;

        if !status.success() {
            bail!("overlayfs mount failed with status {status}");
        }

        Ok(Self {
            root,
            merged,
            mounted: true,
        })
    }

    pub fn path(&self) -> &Path {
        &self.merged
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.mounted {
            let status = Command::new("umount")
                .arg(&self.merged)
                .status()
                .context("running umount")?;

            if !status.success() {
                bail!("umount failed with status {status}");
            }

            self.mounted = false;
        }

        if self.root.exists() {
            std::fs::remove_dir_all(&self.root)
                .with_context(|| format!("removing {}", self.root.display()))?;
        }

        Ok(())
    }
}

impl Drop for OverlayWorkspace {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub fn overlay_available() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    if Path::new("/sys/module/overlay").exists() {
        return true;
    }
    std::fs::read_to_string("/proc/filesystems").is_ok_and(|s| s.contains("overlay"))
}
