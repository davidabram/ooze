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

/// Order candidates by strategy. A seed adds deterministic reordering without
/// destroying strategy intent: seeded Discovery is a deterministic shuffle;
/// seeded score strategies keep their score ordering and use the seed only to
/// break ties among equal-score candidates. Without a seed, behavior is
/// unchanged (discovery order / id tie-breaks).
pub fn order(
    strategy: MutationStrategy,
    candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
    seed: Option<&str>,
) -> Vec<MutationCandidate> {
    match strategy {
        MutationStrategy::Discovery => match seed {
            Some(seed) => shuffle_seeded(candidates, seed),
            None => candidates,
        },
        MutationStrategy::Actionable => rank_actionable(candidates, crap_entries, seed),
        MutationStrategy::HighestCrap => rank_highest_crap(candidates, crap_entries, seed),
    }
}

/// FNV-1a 64-bit. Implemented here so seeded ordering never depends on the
/// standard library's unspecified/randomized hashers.
fn stable_hash64(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn seeded_key(seed: &str, candidate_id: &str) -> u64 {
    stable_hash64(&format!("{seed}\0{candidate_id}"))
}

fn shuffle_seeded(mut candidates: Vec<MutationCandidate>, seed: &str) -> Vec<MutationCandidate> {
    candidates.sort_by(|a, b| {
        seeded_key(seed, &a.id)
            .cmp(&seeded_key(seed, &b.id))
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates
}

// Tie-break for equal-score candidates: seeded hash when a seed is present
// (id as a final guard against hash collisions), plain id otherwise.
fn tie_break(seed: Option<&str>, a: &MutationCandidate, b: &MutationCandidate) -> std::cmp::Ordering {
    match seed {
        Some(seed) => seeded_key(seed, &a.id)
            .cmp(&seeded_key(seed, &b.id))
            .then_with(|| a.id.cmp(&b.id)),
        None => a.id.cmp(&b.id),
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
    index.get(&(c.file.clone(), c.function.clone())).copied()
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
            let s = entry.map_or(i32::MIN, |e| e.crap.round() as i32);
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
    seed: Option<&str>,
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let sa = actionable_score(lookup(&index, a), &DEFAULT_ACTIONABLE_POLICY);
        let sb = actionable_score(lookup(&index, b), &DEFAULT_ACTIONABLE_POLICY);
        sb.cmp(&sa).then_with(|| tie_break(seed, a, b))
    });

    candidates
}

fn rank_highest_crap(
    mut candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
    seed: Option<&str>,
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let ca = lookup(&index, a).map_or(f64::NEG_INFINITY, |e| e.crap);
        let cb = lookup(&index, b).map_or(f64::NEG_INFINITY, |e| e.crap);
        cb.partial_cmp(&ca)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| tie_break(seed, a, b))
    });

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Language, OperatorCategory, OperatorName};

    fn candidate(id: &str, file: &str) -> MutationCandidate {
        MutationCandidate {
            id: id.to_string(),
            file: PathBuf::from(file),
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

    fn ids(candidates: &[MutationCandidate]) -> Vec<&str> {
        candidates.iter().map(|c| c.id.as_str()).collect()
    }

    fn discovery_list() -> Vec<MutationCandidate> {
        (0..8)
            .map(|i| candidate(&format!("m{i}"), "x.rs"))
            .collect()
    }

    #[test]
    fn stable_hash64_matches_fnv1a_vectors() {
        // Drift guards: FNV-1a 64 offset basis and a published test vector.
        assert_eq!(stable_hash64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash64("abc"), 0xe71f_a219_0541_574b);
    }

    #[test]
    fn seeded_discovery_is_reproducible_and_seed_sensitive() {
        let a = order(MutationStrategy::Discovery, discovery_list(), &[], Some("s1"));
        let b = order(MutationStrategy::Discovery, discovery_list(), &[], Some("s1"));
        assert_eq!(ids(&a), ids(&b), "same seed gives same ordering");

        let c = order(MutationStrategy::Discovery, discovery_list(), &[], Some("s2"));
        assert_ne!(ids(&a), ids(&c), "different seed gives different ordering");
    }

    #[test]
    fn unseeded_discovery_preserves_input_order() {
        let out = order(MutationStrategy::Discovery, discovery_list(), &[], None);
        assert_eq!(ids(&out), ["m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7"]);
    }

    #[test]
    fn seeded_score_strategy_keeps_score_buckets() {
        // high.rs outranks low.rs on CRAP; the seed may only reorder within
        // each equal-score bucket, never across buckets.
        let candidates = vec![
            candidate("low-1", "low.rs"),
            candidate("low-2", "low.rs"),
            candidate("high-1", "high.rs"),
            candidate("high-2", "high.rs"),
        ];
        let entries = vec![
            CrapEntry {
                file: PathBuf::from("low.rs"),
                language: Language::Rust,
                function: "f".to_string(),
                line: 1,
                cyclomatic: 1,
                coverage: 0.0,
                crap: 2.0,
            },
            CrapEntry {
                file: PathBuf::from("high.rs"),
                language: Language::Rust,
                function: "f".to_string(),
                line: 1,
                cyclomatic: 1,
                coverage: 0.0,
                crap: 50.0,
            },
        ];

        for seed in ["s1", "s2", "s3"] {
            let out = order(
                MutationStrategy::HighestCrap,
                candidates.clone(),
                &entries,
                Some(seed),
            );
            assert!(
                ids(&out)[..2].iter().all(|id| id.starts_with("high-")),
                "seed {seed} leaked a low-CRAP candidate into the top bucket: {:?}",
                ids(&out)
            );
        }
    }

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
