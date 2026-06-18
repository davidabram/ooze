use crate::core::{AppliedMutation, MutantOutcome, MutantStatus, MutationCandidate};
use anyhow::{Context, Result, bail};
use similar::{ChangeTag, TextDiff};
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;
use walkdir::WalkDir;

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
        let relative_file = candidate
            .file
            .strip_prefix(repo_root)
            .unwrap_or(&candidate.file);

        let workspace_file = self.path().join(relative_file);

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

        let mut mutated = String::with_capacity(
            original.len() - (end - start) + candidate.replacement.len(),
        );
        mutated.push_str(&original[..start]);
        mutated.push_str(&candidate.replacement);
        mutated.push_str(&original[end..]);

        std::fs::write(&workspace_file, &mutated)
            .with_context(|| format!("writing workspace file {}", workspace_file.display()))?;

        let diff = unified_diff(
            &relative_file.to_string_lossy(),
            &original,
            &mutated,
        );

        Ok(AppliedMutation {
            candidate: candidate.clone(),
            workspace_file,
            diff,
        })
    }

    pub fn run_probe(
        &self,
        applied: AppliedMutation,
        probe: &[String],
    ) -> Result<MutantOutcome> {
        if probe.is_empty() {
            bail!("probe command is empty");
        }

        let started = Instant::now();

        let output = Command::new(&probe[0])
            .args(&probe[1..])
            .current_dir(self.path())
            .output()
            .with_context(|| format!("running probe command {:?}", probe))?;

        let duration_ms = started.elapsed().as_millis();
        let exit_code = output.status.code();

        let status = if output.status.success() {
            MutantStatus::Survived
        } else {
            MutantStatus::Killed
        };

        Ok(MutantOutcome {
            candidate: applied.candidate,
            status,
            exit_code,
            duration_ms,
            diff: applied.diff,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
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
        ".git" | "target" | "node_modules" | ".direnv" | ".idea" | ".vscode"
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
