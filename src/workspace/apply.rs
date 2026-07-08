//! Materializing a mutation candidate into a workspace file.

use crate::core::{AppliedMutation, MutationCandidate};
use anyhow::{Context, Result, bail};
use similar::{ChangeTag, TextDiff};
use std::path::Path;

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
