use crate::core::{CrapEntry, MutationCandidate};
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;

/// Tunable thresholds for the `Actionable` strategy. Pulling these out of the
/// ranking code keeps the heuristic explicit and testable, and leaves a single
/// place for `ooze.toml` to override scoring later without touching the
/// scheduler's structure.
#[derive(Debug, Clone)]
pub struct ActionablePolicy {
    /// CRAP range considered the sweet spot for adding tests.
    pub crap: RangeInclusive<f64>,
    pub crap_in_range_score: i32,
    /// Cyclomatic complexity range worth targeting.
    pub cyclomatic: RangeInclusive<usize>,
    pub cyclomatic_in_range_score: i32,
    /// Coverage range where a surviving mutant is most informative.
    pub coverage: RangeInclusive<f64>,
    pub coverage_in_range_score: i32,
    /// Penalty for a function with no coverage at all.
    pub uncovered_penalty: i32,
    /// CRAP above this is too tangled to be a good first target.
    pub huge_crap_ceiling: f64,
    pub huge_crap_penalty: i32,
    /// Score for a candidate whose function has no CRAP entry.
    pub missing_entry_score: i32,
}

pub const DEFAULT_ACTIONABLE_POLICY: ActionablePolicy = ActionablePolicy {
    crap: 15.0..=40.0,
    crap_in_range_score: 100,
    cyclomatic: 5..=15,
    cyclomatic_in_range_score: 50,
    coverage: 60.0..=90.0,
    coverage_in_range_score: 50,
    uncovered_penalty: -80,
    huge_crap_ceiling: 80.0,
    huge_crap_penalty: -50,
    missing_entry_score: -80,
};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum MutationStrategy {
    Discovery,
    Actionable,
    HighestCrap,
}

pub fn order(
    strategy: MutationStrategy,
    candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    match strategy {
        MutationStrategy::Discovery => candidates,
        MutationStrategy::Actionable => rank_actionable(candidates, crap_entries),
        MutationStrategy::HighestCrap => rank_highest_crap(candidates, crap_entries),
    }
}

type CrapIndex<'a> = HashMap<(PathBuf, String), &'a CrapEntry>;

fn index_crap(crap_entries: &[CrapEntry]) -> CrapIndex<'_> {
    crap_entries
        .iter()
        .map(|e| ((e.file.clone(), e.function.clone()), e))
        .collect()
}

fn lookup<'a>(index: &'a CrapIndex<'a>, c: &MutationCandidate) -> Option<&'a CrapEntry> {
    index
        .get(&(c.file.clone(), c.function.clone()))
        .copied()
}

fn actionable_score_with_reasons(
    entry: Option<&CrapEntry>,
    policy: &ActionablePolicy,
) -> (i32, Vec<String>) {
    let Some(e) = entry else {
        return (
            policy.missing_entry_score,
            vec!["no CRAP entry for function".to_string()],
        );
    };

    let mut score = 0;
    let mut reasons = Vec::new();

    if policy.crap.contains(&e.crap) {
        score += policy.crap_in_range_score;
        reasons.push(format!(
            "CRAP {:.1} in actionable range [{},{}]",
            e.crap,
            policy.crap.start(),
            policy.crap.end()
        ));
    }
    if policy.cyclomatic.contains(&e.cyclomatic) {
        score += policy.cyclomatic_in_range_score;
        reasons.push(format!(
            "cyclomatic {} in actionable range [{},{}]",
            e.cyclomatic,
            policy.cyclomatic.start(),
            policy.cyclomatic.end()
        ));
    }
    if policy.coverage.contains(&e.coverage) {
        score += policy.coverage_in_range_score;
        reasons.push(format!(
            "coverage {:.1}% in actionable range [{},{}]",
            e.coverage,
            policy.coverage.start(),
            policy.coverage.end()
        ));
    }
    if e.coverage <= 0.0 {
        score += policy.uncovered_penalty;
        reasons.push("uncovered function".to_string());
    }
    if e.crap > policy.huge_crap_ceiling {
        score += policy.huge_crap_penalty;
        reasons.push(format!(
            "CRAP {:.1} above actionable ceiling {}",
            e.crap, policy.huge_crap_ceiling
        ));
    }

    (score, reasons)
}

fn actionable_score(entry: Option<&CrapEntry>, policy: &ActionablePolicy) -> i32 {
    actionable_score_with_reasons(entry, policy).0
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SelectionExplanation {
    pub selection_score: i32,
    pub selection_reasons: Vec<String>,
    pub crap: Option<f64>,
    pub cyclomatic: Option<usize>,
    pub coverage: Option<f64>,
}

pub fn explain(
    strategy: MutationStrategy,
    candidate: &MutationCandidate,
    crap_entries: &[CrapEntry],
) -> SelectionExplanation {
    let index = index_crap(crap_entries);
    let entry = lookup(&index, candidate);

    let (score, mut reasons) = match strategy {
        MutationStrategy::Discovery => (0, vec!["discovery order (no scoring)".to_string()]),
        MutationStrategy::Actionable => {
            actionable_score_with_reasons(entry, &DEFAULT_ACTIONABLE_POLICY)
        }
        MutationStrategy::HighestCrap => {
            let s = entry
                .map_or(i32::MIN, |e| e.crap.round() as i32);
            let r = match entry {
                Some(e) => vec![format!("CRAP {:.1}", e.crap)],
                None => vec!["no CRAP entry for function".to_string()],
            };
            (s, r)
        }
    };

    if reasons.is_empty() {
        reasons.push("no scoring rules matched".to_string());
    }

    SelectionExplanation {
        selection_score: score,
        selection_reasons: reasons,
        crap: entry.map(|e| e.crap),
        cyclomatic: entry.map(|e| e.cyclomatic),
        coverage: entry.map(|e| e.coverage),
    }
}

fn rank_actionable(
    mut candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let sa = actionable_score(lookup(&index, a), &DEFAULT_ACTIONABLE_POLICY);
        let sb = actionable_score(lookup(&index, b), &DEFAULT_ACTIONABLE_POLICY);
        sb.cmp(&sa).then_with(|| a.id.cmp(&b.id))
    });

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Language;

    fn entry(cyclomatic: usize, coverage: f64, crap: f64) -> CrapEntry {
        CrapEntry {
            file: PathBuf::from("x.rs"),
            language: Language::Rust,
            function: "f".to_string(),
            line: 1,
            cyclomatic,
            coverage,
            crap,
        }
    }

    #[test]
    fn default_policy_reproduces_known_scores() {
        let p = &DEFAULT_ACTIONABLE_POLICY;
        // No entry: missing-entry score.
        assert_eq!(actionable_score(None, p), -80);
        // Squarely actionable: all three ranges hit (100 + 50 + 50).
        assert_eq!(actionable_score(Some(&entry(10, 75.0, 25.0)), p), 200);
        // Uncovered: CRAP + cyclomatic ranges hit, coverage range misses, and
        // the uncovered penalty applies (100 + 50 - 80).
        assert_eq!(actionable_score(Some(&entry(10, 0.0, 25.0)), p), 70);
        // Tangled: above the CRAP ceiling, outside all positive ranges.
        assert_eq!(actionable_score(Some(&entry(20, 95.0, 90.0)), p), -50);
    }
}

fn rank_highest_crap(
    mut candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let ca = lookup(&index, a).map_or(f64::NEG_INFINITY, |e| e.crap);
        let cb = lookup(&index, b).map_or(f64::NEG_INFINITY, |e| e.crap);
        cb.partial_cmp(&ca)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    candidates
}
