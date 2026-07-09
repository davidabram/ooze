//! Mutation plan construction shared by `plan-mutants` and `test-mutants`.
//!
//! [`build_plan`] runs the whole selection pipeline — scan, discover, static
//! skips, `--changed-only` filtering, coverage scoring, strategy ordering, and
//! `--limit` — and returns a typed [`BuiltPlan`]. It never renders output,
//! writes files, or exits; command handlers own all presentation.

use std::path::PathBuf;

use anyhow::Context;

use crate::{core, crap, lang, mutate, scheduler, skip, source_path};

pub(crate) const DEFAULT_EXCLUDES: &[&str] = &[
    "target/**",
    ".ooze/**",
    ".git/**",
    "node_modules/**",
    "vendor/**",
    "__pycache__/**",
    ".gradle/**",
];

/// Inputs to [`build_plan`]. `excludes` must already be fully resolved (see
/// [`resolve_excludes`]); the pipeline applies them as-is.
pub(crate) struct PlanOptions {
    pub path: PathBuf,
    pub excludes: Vec<String>,
    pub filter: mutate::OperatorFilter,
    pub strategy: scheduler::MutationStrategy,
    pub limit: Option<usize>,
    /// Deterministic ordering seed; `None` keeps today's unseeded ordering.
    pub seed: Option<String>,
    pub changed_only: Option<String>,
    pub no_static_skips: bool,
    pub coverage: Vec<String>,
    pub lcov: Option<PathBuf>,
}

/// How `--changed-only` narrowed the candidate set, for diagnostics.
pub(crate) struct ChangedOnlyStats {
    pub base: String,
    /// Non-skipped candidates before the changed-files filter.
    pub before: usize,
    /// Non-skipped candidates in changed files.
    pub kept: usize,
}

/// The fully-selected mutation plan: `candidates` is ordered by strategy and
/// truncated to the limit, ready to hand to the runner or serialize as a plan.
pub(crate) struct BuiltPlan {
    pub crap_entries: Vec<core::CrapEntry>,
    pub candidates: Vec<core::MutationCandidate>,
    pub skipped_candidates: Vec<skip::SkippedCandidate>,
    /// Candidate count after `--changed-only` but before static skips.
    pub total_candidates_before_static_skips: usize,
    pub strategy: scheduler::MutationStrategy,
    /// The seed the plan was ordered with, echoed back for plan output.
    pub seed: Option<String>,
    pub excludes: Vec<String>,
    pub operator_filter: mutate::OperatorFilterReport,
    pub changed_only: Option<ChangedOnlyStats>,
    pub coverage_diagnostics: Option<CoverageDiagnostics>,
}

/// Build the mutation execution plan for `options`.
pub(crate) fn build_plan(options: PlanOptions) -> anyhow::Result<BuiltPlan> {
    let PlanOptions {
        path,
        excludes,
        filter,
        strategy,
        limit,
        seed,
        changed_only,
        no_static_skips,
        coverage,
        lcov,
    } = options;

    let registry = lang::CompiledRegistry::compile(lang::supported_languages(), &filter)?;
    let functions = lang::scan_directory_with_registry(&registry, &path, &excludes)?;
    let candidates = mutate::discover_mutants(&functions, &registry)?;

    let (kept, skipped) = if no_static_skips {
        (candidates, Vec::new())
    } else {
        skip::partition(candidates)
    };

    let (kept, skipped, changed_stats) = match changed_only.as_deref() {
        Some(base) => {
            let changed = git_changed_files(base, &path)?;
            let (kept, skipped, stats) = apply_changed_filter(kept, skipped, &changed, base);
            (kept, skipped, Some(stats))
        }
        None => (kept, skipped, None),
    };

    let total_candidates_before_static_skips = kept.len() + skipped.len();

    let coverage = resolve_coverage(&coverage, lcov.as_deref())?;
    let (crap_entries, coverage_diagnostics) = score_with_optional_coverage(functions, coverage);

    let candidates = order_and_limit(strategy, kept, &crap_entries, limit, seed.as_deref());

    Ok(BuiltPlan {
        crap_entries,
        candidates,
        skipped_candidates: skipped,
        total_candidates_before_static_skips,
        strategy,
        seed,
        excludes,
        operator_filter: (&filter).into(),
        changed_only: changed_stats,
        coverage_diagnostics,
    })
}

/// Order candidates by strategy (seeded when a seed is given), then truncate
/// to the limit. The limit is applied after ordering so it selects the
/// top-ranked candidates.
fn order_and_limit(
    strategy: scheduler::MutationStrategy,
    candidates: Vec<core::MutationCandidate>,
    crap_entries: &[core::CrapEntry],
    limit: Option<usize>,
    seed: Option<&str>,
) -> Vec<core::MutationCandidate> {
    let mut ordered = scheduler::order(strategy, candidates, crap_entries, seed);
    if let Some(limit) = limit {
        ordered.truncate(limit);
    }
    ordered
}

// Narrows both kept and skipped candidates to changed files, recording how the
// kept set shrank for the `--changed-only` diagnostic.
fn apply_changed_filter(
    kept: Vec<core::MutationCandidate>,
    skipped: Vec<skip::SkippedCandidate>,
    changed: &std::collections::HashSet<source_path::SourcePath>,
    base: &str,
) -> (
    Vec<core::MutationCandidate>,
    Vec<skip::SkippedCandidate>,
    ChangedOnlyStats,
) {
    let before = kept.len();
    let kept = filter_candidates_to_changed(kept, changed);
    let skipped = skipped
        .into_iter()
        .filter(|s| is_changed(&s.candidate, changed))
        .collect();
    let stats = ChangedOnlyStats {
        base: base.to_string(),
        before,
        kept: kept.len(),
    };
    (kept, skipped, stats)
}

fn is_changed(
    candidate: &core::MutationCandidate,
    changed: &std::collections::HashSet<source_path::SourcePath>,
) -> bool {
    source_path::SourcePath::canonical(&candidate.file).is_some_and(|id| changed.contains(&id))
}

// Keeps only candidates whose source file is among `changed`. Candidate files
// that fail to canonicalize (already gone) are dropped.
fn filter_candidates_to_changed(
    candidates: Vec<core::MutationCandidate>,
    changed: &std::collections::HashSet<source_path::SourcePath>,
) -> Vec<core::MutationCandidate> {
    candidates
        .into_iter()
        .filter(|c| is_changed(c, changed))
        .collect()
}

// Collects the set of files that differ from `base`, used by `--changed-only`.
// Returns canonical absolute paths so they can be matched against candidate
// file paths regardless of how `--path` was spelled. The union covers three
// sources: commits on this branch since the merge-base with `base`, working-tree
// modifications (staged and unstaged), and untracked-but-not-ignored files.
fn git_changed_files(
    base: &str,
    root: &std::path::Path,
) -> anyhow::Result<std::collections::HashSet<source_path::SourcePath>> {
    let toplevel_out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running `git rev-parse --show-toplevel`")?;
    if !toplevel_out.status.success() {
        anyhow::bail!(
            "--changed-only: `git rev-parse` failed (is {} inside a git repo?): {}",
            root.display(),
            String::from_utf8_lossy(&toplevel_out.stderr).trim()
        );
    }
    let toplevel = PathBuf::from(String::from_utf8_lossy(&toplevel_out.stdout).trim());

    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    collect_git_paths(
        root,
        &["diff", "--name-only", &format!("{base}...HEAD")],
        &mut names,
    )?;
    collect_git_paths(root, &["diff", "--name-only", "HEAD"], &mut names)?;
    collect_git_paths(
        root,
        &["ls-files", "--others", "--exclude-standard"],
        &mut names,
    )?;

    // Resolve to source identities; drop entries that no longer exist (e.g.
    // deletions) since they carry no mutation candidates anyway.
    let mut out = std::collections::HashSet::new();
    for name in names {
        if let Some(id) = source_path::SourcePath::under(&toplevel, std::path::Path::new(&name)) {
            out.insert(id);
        }
    }
    Ok(out)
}

fn parse_output_lines(stdout: &[u8], out: &mut std::collections::HashSet<String>) {
    for line in String::from_utf8_lossy(stdout).lines() {
        let line = line.trim();
        if !line.is_empty() {
            out.insert(line.to_string());
        }
    }
}

fn collect_git_paths(
    root: &std::path::Path,
    args: &[&str],
    out: &mut std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "--changed-only: `git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    parse_output_lines(&output.stdout, out);
    Ok(())
}

fn read_gitignore_patterns(root: &std::path::Path) -> Vec<String> {
    let path = root.join(".gitignore");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches('/').to_string())
        .collect()
}

pub(crate) fn resolve_excludes(root: &std::path::Path, user: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEFAULT_EXCLUDES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    out.extend(read_gitignore_patterns(root));
    out.extend(user.iter().cloned());
    out
}

/// Coverage resolved from the CLI, ready for scoring plus a count of how many
/// reports were merged (for diagnostics).
pub(crate) struct ResolvedCoverage {
    map: std::collections::HashMap<PathBuf, core::FileCoverage>,
    reports: usize,
}

/// How the coverage reports lined up with the scanned source tree. Command
/// handlers surface this to the user; planning only computes it.
pub(crate) struct CoverageDiagnostics {
    pub reports: usize,
    pub matches: crap::CoverageMatch,
}

/// Resolve coverage from the (repeatable) `--coverage` specs, falling back to
/// the deprecated `--lcov` flag. Returns `None` when neither was supplied.
pub(crate) fn resolve_coverage(
    coverage: &[String],
    lcov: Option<&std::path::Path>,
) -> anyhow::Result<Option<ResolvedCoverage>> {
    use crap::coverage::{CoverageFormat, CoverageInput};

    // `--coverage` specs take precedence; the deprecated `--lcov` flag is just an
    // implicit lcov-format input. Each spec is parsed to a typed input once here.
    let inputs: Vec<CoverageInput> = if !coverage.is_empty() {
        coverage
            .iter()
            .map(|spec| CoverageInput::parse(spec))
            .collect::<anyhow::Result<_>>()?
    } else if let Some(path) = lcov {
        vec![CoverageInput {
            format: CoverageFormat::Lcov,
            path: path.to_path_buf(),
        }]
    } else {
        return Ok(None);
    };

    Ok(Some(ResolvedCoverage {
        reports: inputs.len(),
        map: crap::coverage::load_inputs(&inputs)?,
    }))
}

/// Score `functions` against resolved coverage when present, or without
/// coverage otherwise. With coverage, also returns match diagnostics.
pub(crate) fn score_with_optional_coverage(
    functions: Vec<core::FunctionSpan>,
    coverage: Option<ResolvedCoverage>,
) -> (Vec<core::CrapEntry>, Option<CoverageDiagnostics>) {
    match coverage {
        Some(ResolvedCoverage { map, reports }) => {
            let mut scanned: Vec<PathBuf> = functions.iter().map(|f| f.file.clone()).collect();
            scanned.sort();
            scanned.dedup();
            let diagnostics = CoverageDiagnostics {
                reports,
                matches: crap::match_report(&scanned, &map),
            };
            (
                crap::score_with_coverage(functions, &map),
                Some(diagnostics),
            )
        }
        None => (crap::score_without_coverage(functions), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Language, MutationCandidate, OperatorCategory, OperatorName};

    fn candidate(id: &str, file: PathBuf) -> MutationCandidate {
        MutationCandidate {
            id: id.to_string(),
            file,
            language: Language::Rust,
            function: "f".to_string(),
            operator: OperatorName::SwapBoolean,
            operator_category: OperatorCategory::BooleanLiteral,
            implementation: "rust.swap_boolean".to_string(),
            line: 1,
            column: 1,
            start_byte: 0,
            end_byte: 1,
            original: "true".to_string(),
            replacement: "false".to_string(),
            description: String::new(),
        }
    }

    fn crap_entry(file: &std::path::Path, crap: f64) -> core::CrapEntry {
        core::CrapEntry {
            file: file.to_path_buf(),
            language: Language::Rust,
            function: "f".to_string(),
            line: 1,
            cyclomatic: 1,
            coverage: 0.0,
            crap,
        }
    }

    // --- order_and_limit ----------------------------------------------------

    #[test]
    fn limit_is_applied_after_ordering() {
        let low = std::path::Path::new("low.rs");
        let high = std::path::Path::new("high.rs");
        // Discovery order puts the low-CRAP candidate first; highest-crap
        // ordering must reverse that before the limit cuts to one.
        let candidates = vec![
            candidate("m-low", low.to_path_buf()),
            candidate("m-high", high.to_path_buf()),
        ];
        let entries = vec![crap_entry(low, 1.0), crap_entry(high, 50.0)];

        let out = order_and_limit(
            scheduler::MutationStrategy::HighestCrap,
            candidates,
            &entries,
            Some(1),
            None,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "m-high");
    }

    #[test]
    fn order_and_limit_without_limit_keeps_everything() {
        let file = std::path::Path::new("x.rs");
        let candidates = vec![
            candidate("a", file.to_path_buf()),
            candidate("b", file.to_path_buf()),
        ];
        let out = order_and_limit(
            scheduler::MutationStrategy::Discovery,
            candidates,
            &[],
            None,
            None,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "a"); // discovery preserves input order
    }

    #[test]
    fn seeded_limit_selects_head_of_seeded_ordering() {
        let file = std::path::Path::new("x.rs");
        let list = || -> Vec<MutationCandidate> {
            (0..6)
                .map(|i| candidate(&format!("m{i}"), file.to_path_buf()))
                .collect()
        };
        let full = order_and_limit(
            scheduler::MutationStrategy::Discovery,
            list(),
            &[],
            None,
            Some("abc"),
        );
        let limited = order_and_limit(
            scheduler::MutationStrategy::Discovery,
            list(),
            &[],
            Some(2),
            Some("abc"),
        );
        // The limit truncates the seeded ordering, not the discovery ordering.
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, full[0].id);
        assert_eq!(limited[1].id, full[1].id);
        assert_ne!(
            (limited[0].id.as_str(), limited[1].id.as_str()),
            ("m0", "m1"),
            "seed \"abc\" should not happen to preserve discovery order for this list"
        );
    }

    // --- apply_changed_filter -----------------------------------------------

    #[test]
    fn apply_changed_filter_narrows_kept_and_skipped_and_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let changed_file = tmp.path().join("changed.rs");
        let other_file = tmp.path().join("other.rs");
        std::fs::write(&changed_file, "fn a() {}").unwrap();
        std::fs::write(&other_file, "fn b() {}").unwrap();

        let mut changed = std::collections::HashSet::new();
        changed.insert(source_path::SourcePath::canonical(&changed_file).unwrap());

        let kept = vec![
            candidate("in-changed", changed_file.clone()),
            candidate("in-other", other_file.clone()),
        ];
        let skipped = vec![
            skip::SkippedCandidate {
                candidate: candidate("skip-changed", changed_file),
                skip_rule: "r",
                skip_reason: String::new(),
            },
            skip::SkippedCandidate {
                candidate: candidate("skip-other", other_file),
                skip_rule: "r",
                skip_reason: String::new(),
            },
        ];

        let (kept, skipped, stats) = apply_changed_filter(kept, skipped, &changed, "main");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "in-changed");
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].candidate.id, "skip-changed");
        assert_eq!(stats.base, "main");
        assert_eq!(stats.before, 2);
        assert_eq!(stats.kept, 1);
    }

    #[test]
    fn filter_candidates_to_changed_drops_missing_files() {
        let changed = std::collections::HashSet::new();
        let out = filter_candidates_to_changed(
            vec![candidate("gone", PathBuf::from("/nonexistent/file.rs"))],
            &changed,
        );
        assert!(out.is_empty());
    }

    // --- resolve_excludes ---------------------------------------------------

    #[test]
    fn resolve_excludes_layers_defaults_gitignore_and_user_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "# comment\n\n/dist\nfoo\n").unwrap();
        let out = resolve_excludes(tmp.path(), &["user/**".to_string()]);

        let defaults: Vec<String> = DEFAULT_EXCLUDES.iter().map(|s| (*s).to_string()).collect();
        assert!(out.starts_with(&defaults));
        assert!(out.contains(&"dist".to_string())); // leading slash stripped
        assert!(out.contains(&"foo".to_string()));
        assert_eq!(out.last().unwrap(), "user/**");
        assert!(!out.iter().any(|p| p.starts_with('#')));
    }

    // --- collect_git_paths / parse_output_lines -----------------------------

    #[test]
    fn parse_output_lines_includes_non_empty_lines() {
        let mut out = std::collections::HashSet::new();
        parse_output_lines(b"src/foo.rs\nsrc/bar.rs\n", &mut out);
        assert!(out.contains("src/foo.rs"));
        assert!(out.contains("src/bar.rs"));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn parse_output_lines_skips_empty_and_whitespace_only_lines() {
        let mut out = std::collections::HashSet::new();
        parse_output_lines(b"src/foo.rs\n\n   \nsrc/bar.rs", &mut out);
        assert_eq!(out.len(), 2, "empty/whitespace lines must not be inserted");
        assert!(out.contains("src/foo.rs"));
        assert!(out.contains("src/bar.rs"));
    }

    #[test]
    fn collect_git_paths_returns_error_when_git_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let mut out = std::collections::HashSet::new();
        // A plain tempdir is not a git repo, so any git command will fail.
        let result = collect_git_paths(tmp.path(), &["diff", "--name-only"], &mut out);
        assert!(result.is_err(), "expected error from failed git command");
    }
}
