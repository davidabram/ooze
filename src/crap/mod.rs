use std::collections::HashMap;
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
        .map(|f| {
            let coverage = 0.0;
            CrapEntry {
                file: f.file,
                language: f.language,
                function: f.name,
                line: f.start_line,
                cyclomatic: f.cyclomatic,
                coverage: None,
                crap: score_crap(f.cyclomatic, coverage),
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

pub fn score_with_coverage(
    functions: Vec<FunctionSpan>,
    coverage: HashMap<PathBuf, FileCoverage>,
) -> Vec<CrapEntry> {
    let mut entries: Vec<CrapEntry> = functions
        .into_iter()
        .map(|f| {
            let coverage_pct = lookup_coverage(&f.file, &coverage)
                .map(|file_cov| file_cov.coverage_in_span(f.start_line, f.end_line));

            let coverage_for_score = coverage_pct.unwrap_or(0.0);

            CrapEntry {
                file: f.file,
                language: f.language,
                function: f.name,
                line: f.start_line,
                cyclomatic: f.cyclomatic,
                coverage: coverage_pct,
                crap: score_crap(f.cyclomatic, coverage_for_score),
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
    if let Some(file_cov) = coverage.get(source_file) {
        return Some(file_cov);
    }

    coverage
        .iter()
        .find_map(|(coverage_path, file_cov)| {
            (path_has_suffix(source_file, coverage_path)
                || path_has_suffix(coverage_path, source_file))
            .then_some(file_cov)
        })
}

fn path_has_suffix(path: &Path, suffix: &Path) -> bool {
    let path_components: Vec<_> = path.components().collect();
    let suffix_components: Vec<_> = suffix.components().collect();

    if suffix_components.len() > path_components.len() {
        return false;
    }

    path_components[path_components.len() - suffix_components.len()..] == suffix_components[..]
}

#[cfg(test)]
mod tests {
    use super::{score_crap, FileCoverage};

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
