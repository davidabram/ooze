use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::{CrapEntry, FileCoverage, FunctionSpan};

pub mod coverage;

#[allow(dead_code)]
pub const DEFAULT_THRESHOLD: f64 = 30.0;

pub fn score_crap(cyclomatic: usize, coverage_pct: f64) -> f64 {
    let complexity = cyclomatic as f64;
    let uncovered = 1.0 - coverage_pct.clamp(0.0, 100.0) / 100.0;

    complexity.powi(2) * uncovered.powi(3) + complexity
}

pub fn score_without_coverage(functions: Vec<FunctionSpan>) -> Vec<CrapEntry> {
    let mut entries: Vec<CrapEntry> = functions
        .into_iter()
        .map(|f| CrapEntry {
            file: f.file,
            language: f.language,
            function: f.name,
            line: f.start_line,
            cyclomatic: f.cyclomatic,
            coverage: 0.0,
            crap: score_crap(f.cyclomatic, 0.0),
        })
        .collect();

    entries.sort_by(|a, b| {
        b.crap
            .partial_cmp(&a.crap)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    entries
}

pub fn score_with_coverage(
    functions: Vec<FunctionSpan>,
    coverage: &HashMap<PathBuf, FileCoverage>,
) -> Vec<CrapEntry> {
    let mut entries: Vec<CrapEntry> = functions
        .into_iter()
        .map(|f| {
            let coverage_pct = lookup_coverage(&f.file, coverage).map_or(0.0, |file_cov| {
                file_cov.coverage_in_span(f.start_line, f.end_line)
            });

            CrapEntry {
                file: f.file,
                language: f.language,
                function: f.name,
                line: f.start_line,
                cyclomatic: f.cyclomatic,
                coverage: coverage_pct,
                crap: score_crap(f.cyclomatic, coverage_pct),
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        b.crap
            .partial_cmp(&a.crap)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    entries
}

fn lookup_coverage<'a>(
    source_file: &Path,
    coverage: &'a HashMap<PathBuf, FileCoverage>,
) -> Option<&'a FileCoverage> {
    match_coverage_key(source_file, coverage).map(|key| &coverage[key])
}

/// Find the coverage entry that corresponds to a scanned source file, returning
/// its key. An exact path wins; otherwise we fall back to suffix matching in
/// either direction (handles relative vs. absolute and package-rooted paths).
///
/// Deliberately *not* keyed on `SourcePath`: coverage keys are whatever the test
/// tool emitted (`pkg/foo.rs`, `github.com/me/app/foo.go`, a CI-absolute path)
/// and frequently do not resolve to a real file under our scan root, so
/// canonical identity would fail to match exactly the cases this fuzzy suffix
/// match exists to handle.
fn match_coverage_key<'a>(
    source_file: &Path,
    coverage: &'a HashMap<PathBuf, FileCoverage>,
) -> Option<&'a PathBuf> {
    if let Some((key, _)) = coverage.get_key_value(source_file) {
        return Some(key);
    }

    coverage.keys().find(|coverage_path| {
        path_has_suffix(source_file, coverage_path) || path_has_suffix(coverage_path, source_file)
    })
}

/// Diagnostics describing how well a coverage map lines up with the scanned
/// source tree. Surfaced to users so path-root mismatches (Docker, CI,
/// monorepos) are visible rather than silently scoring everything as uncovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)] // the shared `_files` suffix aids clarity here
pub struct CoverageMatch {
    /// Distinct source files present in the coverage map.
    pub coverage_source_files: usize,
    /// Scanned source files that matched a coverage entry.
    pub matched_source_files: usize,
    /// Scanned source files with no coverage entry.
    pub unmatched_source_files: usize,
    /// Coverage entries that no scanned source file matched.
    pub unmatched_coverage_files: usize,
}

/// Compare the scanned source files against a coverage map.
pub fn match_report(
    scanned_files: &[PathBuf],
    coverage: &HashMap<PathBuf, FileCoverage>,
) -> CoverageMatch {
    let mut matched_keys: HashSet<&PathBuf> = HashSet::new();
    let mut matched = 0;
    let mut unmatched = 0;

    for file in scanned_files {
        if let Some(key) = match_coverage_key(file, coverage) {
            matched += 1;
            matched_keys.insert(key);
        } else {
            unmatched += 1;
        }
    }

    CoverageMatch {
        coverage_source_files: coverage.len(),
        matched_source_files: matched,
        unmatched_source_files: unmatched,
        unmatched_coverage_files: coverage.len() - matched_keys.len(),
    }
}

fn path_has_suffix(path: &Path, suffix: &Path) -> bool {
    // Ignore `.` (CurDir) components so a scanned `./foo.go` still matches a
    // coverage path like `github.com/me/app/foo.go` or an absolute path.
    let path_components: Vec<_> = path
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .map(std::path::Component::as_os_str)
        .collect();
    let suffix_components: Vec<_> = suffix
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .map(std::path::Component::as_os_str)
        .collect();

    if suffix_components.len() > path_components.len() {
        return false;
    }

    path_components[path_components.len() - suffix_components.len()..] == suffix_components[..]
}

#[cfg(test)]
mod tests {
    use super::{FileCoverage, score_crap};

    #[test]
    fn untested_complex_method_matches_known_example() {
        assert!((score_crap(6, 0.0) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fully_covered_method_scores_as_complexity() {
        assert!((score_crap(6, 100.0) - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn coverage_in_span_counts_hit_executable_lines() {
        let mut fc = FileCoverage::default();

        fc.lines.insert(1, 1);
        fc.lines.insert(2, 1);
        fc.lines.insert(3, 0);
        fc.lines.insert(4, 0);

        assert!((fc.coverage_in_span(1, 4) - 50.0).abs() < f64::EPSILON);
    }
}
