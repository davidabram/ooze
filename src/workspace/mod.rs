//! Workspaces: where and how a mutant is materialized before probing.
//!
//! A workspace is an isolated checkout of the repo that a mutation can be
//! applied to without touching the source tree. Three backends exist: a
//! copy-on-write temp copy, an overlayfs mount, and pooled git worktrees.
//! Process execution over these workspaces lives in `crate::execution`.

use crate::core::{AppliedMutation, MutationCandidate};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

mod apply;
mod copy;
pub mod overlay;
pub mod worktree;

pub use apply::apply_mutation;
use copy::copy_repo;

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
}

pub(crate) fn create_workspace(
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

pub fn default_build_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("build-cache")
}
