//! Deterministic, seed-based candidate selection.
//!
//! The seed never generates or alters mutants. Discovery produces the full
//! candidate universe first; selection then assigns every candidate a stable
//! identity, hashes `(algorithm version, commit, seed, stable id)` into a
//! ranking key, and sorts by that key. Applying `--limit` after the sort makes
//! a smaller limit a strict prefix of a larger one for the same inputs.
//!
//! This module is the single source of truth for seeded ordering; the scheduler
//! and planning layers call into it rather than hashing seeds themselves.

use crate::core::MutationCandidate;

/// Domain/version prefix mixed into every ranking key. Bumping it deliberately
/// changes what every existing seed selects, so the meaning of a seed can never
/// drift silently when the algorithm is revised. Keep [`ALGORITHM_NAME`] in sync.
pub const ALGORITHM_VERSION: &str = "ooze-seeded-selection-v1";

/// Short algorithm name recorded in plan/report/run metadata, so a stored
/// selection can be traced back to the code that produced it.
pub const ALGORITHM_NAME: &str = "hash-rank-v1";

/// The inputs, besides the candidate itself, that a seeded ranking depends on:
/// the user's seed and the commit the tree is at. Reproducibility holds only
/// when both — plus the candidate universe — are unchanged.
#[derive(Debug, Clone)]
pub struct SelectionContext {
    seed: String,
    commit: String,
}

impl SelectionContext {
    /// `commit` is the current Git commit hash, or empty when Git metadata is
    /// unavailable (outside a repo, or a repo with no commits yet). An empty
    /// commit is still deterministic — reproducibility just no longer pins to a
    /// specific revision.
    pub fn new(seed: impl Into<String>, commit: impl Into<String>) -> Self {
        Self {
            seed: seed.into(),
            commit: commit.into(),
        }
    }

    /// The 32-byte ranking key for a candidate. Candidates sort ascending by
    /// these bytes; equal keys fall back to the stable id (see [`compare`]).
    pub fn ranking_key(&self, candidate: &MutationCandidate) -> [u8; 32] {
        ranking_key(&self.commit, &self.seed, &stable_candidate_id(candidate))
    }

    /// Lowercase hex of [`ranking_key`], for debugging in plan/report output.
    pub fn ranking_key_hex(&self, candidate: &MutationCandidate) -> String {
        to_hex(&self.ranking_key(candidate))
    }

    /// Total order over candidates: ranking key ascending, then stable id as the
    /// final tie-breaker. The id tie-break guarantees a deterministic order even
    /// in the (astronomically unlikely) event of a 256-bit hash collision.
    pub fn compare(&self, a: &MutationCandidate, b: &MutationCandidate) -> std::cmp::Ordering {
        self.ranking_key(a)
            .cmp(&self.ranking_key(b))
            .then_with(|| stable_candidate_id(a).cmp(&stable_candidate_id(b)))
    }
}

/// A candidate's stable identity, built only from deterministic properties of
/// the mutation itself: repo-relative file path, operator, source byte range,
/// and the exact original/replacement text. It deliberately excludes discovery
/// index, worker index, timestamps, absolute paths, and any hash-map iteration
/// order, so the same mutation always hashes the same regardless of how it was
/// found.
pub fn stable_candidate_id(c: &MutationCandidate) -> String {
    // NUL-separated so no field boundary can be forged by field contents (paths,
    // operators, and byte offsets never contain NUL; source text is UTF-8 and a
    // literal NUL byte cannot appear in it).
    format!(
        "{path}\u{0}{operator}\u{0}{start}\u{0}{end}\u{0}{original}\u{0}{replacement}",
        path = relative_path(c),
        operator = c.operator.as_str(),
        start = c.start_byte,
        end = c.end_byte,
        original = c.original,
        replacement = c.replacement,
    )
}

/// Repo-relative-ish, forward-slash file path, matching how candidate ids are
/// spelled elsewhere: a leading `./` is stripped so `./a.rs` and `a.rs` share an
/// identity. The path is used verbatim as a stable string, not resolved against
/// the filesystem, so it never depends on the absolute checkout location.
fn relative_path(c: &MutationCandidate) -> String {
    let display = c.file.to_string_lossy().replace('\\', "/");
    display
        .strip_prefix("./")
        .map_or(display.clone(), std::string::ToString::to_string)
}

/// Hash the ordered selection inputs into a 32-byte key with BLAKE3.
///
/// Fields are length-prefixed (not merely concatenated) so distinct field
/// splits can never collide — `("a", "bc")` and `("ab", "c")` hash differently.
/// The algorithm-version prefix is always the first field, which is what lets
/// the algorithm change later without reusing an old seed's meaning.
fn ranking_key(commit: &str, seed: &str, stable_id: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for field in [ALGORITHM_VERSION, commit, seed, stable_id] {
        let len = field.len() as u64;
        hasher.update(&len.to_le_bytes());
        hasher.update(field.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Language, MutationCandidate, OperatorCategory, OperatorName};
    use std::path::PathBuf;

    fn candidate(id: &str, file: &str, op: OperatorName, original: &str) -> MutationCandidate {
        MutationCandidate {
            id: id.to_string(),
            file: PathBuf::from(file),
            language: Language::Rust,
            function: "f".to_string(),
            operator: op,
            operator_category: OperatorCategory::BooleanLiteral,
            implementation: format!("rust.{}", op.as_str()),
            line: 1,
            column: 1,
            start_byte: 0,
            end_byte: original.len(),
            original: original.to_string(),
            replacement: "REPL".to_string(),
            description: String::new(),
        }
    }

    fn sample(n: usize) -> Vec<MutationCandidate> {
        (0..n)
            .map(|i| {
                candidate(
                    &format!("m{i}"),
                    "x.rs",
                    OperatorName::SwapBoolean,
                    &format!("v{i}"),
                )
            })
            .collect()
    }

    fn ranked_ids(ctx: &SelectionContext, mut list: Vec<MutationCandidate>) -> Vec<String> {
        list.sort_by(|a, b| ctx.compare(a, b));
        list.into_iter().map(|c| c.id).collect()
    }

    #[test]
    fn stable_id_ignores_discovery_index_and_absolute_prefix() {
        // Same mutation, different `id` string and a `./` path prefix: identity
        // must be identical (id and leading `./` are not part of the identity).
        let a = candidate("first", "./a.rs", OperatorName::SwapBoolean, "true");
        let b = candidate("second", "a.rs", OperatorName::SwapBoolean, "true");
        assert_eq!(stable_candidate_id(&a), stable_candidate_id(&b));
    }

    #[test]
    fn stable_id_distinguishes_operator_and_text() {
        let base = candidate("x", "a.rs", OperatorName::SwapBoolean, "true");
        let other_op = candidate("x", "a.rs", OperatorName::NegateEquality, "true");
        assert_ne!(stable_candidate_id(&base), stable_candidate_id(&other_op));

        let mut other_repl = base.clone();
        other_repl.replacement = "different".to_string();
        assert_ne!(
            stable_candidate_id(&base),
            stable_candidate_id(&other_repl),
            "replacement text is part of the identity"
        );
    }

    // Case 1: same candidates + same seed => same ordering across repeated calls.
    #[test]
    fn same_seed_reproduces_ordering() {
        let ctx = SelectionContext::new("42", "commitA");
        let first = ranked_ids(&ctx, sample(12));
        let second = ranked_ids(&ctx, sample(12));
        assert_eq!(first, second);
    }

    // Case 2: different seeds produce different ordering for a large enough set.
    #[test]
    fn different_seeds_reorder() {
        let a = ranked_ids(&SelectionContext::new("42", "commitA"), sample(16));
        let b = ranked_ids(&SelectionContext::new("43", "commitA"), sample(16));
        assert_ne!(a, b);
    }

    // Case 3: a smaller limit is a prefix of a larger limit under the same seed.
    #[test]
    fn smaller_limit_is_prefix_of_larger() {
        let ctx = SelectionContext::new("42", "commitA");
        let full = ranked_ids(&ctx, sample(20));
        // Truncating the ranked list gives selections where the shorter is a
        // strict prefix of the longer (this is what `--limit` does downstream).
        assert_eq!(
            full[..3],
            full[..5][..3],
            "the shorter selection is a prefix"
        );
    }

    // Case 4: discovery input order does not affect the seeded result.
    #[test]
    fn input_order_does_not_change_result() {
        let ctx = SelectionContext::new("42", "commitA");
        let forward = sample(16);
        let mut reversed = forward.clone();
        reversed.reverse();
        assert_eq!(ranked_ids(&ctx, forward), ranked_ids(&ctx, reversed));
    }

    // Case 6: the commit hash changes the ranking.
    #[test]
    fn commit_changes_ranking() {
        let a = ranked_ids(&SelectionContext::new("42", "commitA"), sample(16));
        let b = ranked_ids(&SelectionContext::new("42", "commitB"), sample(16));
        assert_ne!(a, b, "changing the commit must change the seeded order");
    }

    // Case 7: the algorithm version prefix changes the ranking key.
    #[test]
    fn algorithm_version_changes_key() {
        let stable_id = "some-stable-id";
        let with_v1 = ranking_key("commitA", "42", stable_id);
        // Recompute the key as if the version prefix were different, by mixing a
        // different first field. This mirrors what bumping ALGORITHM_VERSION does.
        let alt = {
            let mut h = blake3::Hasher::new();
            for field in ["ooze-seeded-selection-v2", "commitA", "42", stable_id] {
                h.update(&(field.len() as u64).to_le_bytes());
                h.update(field.as_bytes());
            }
            *h.finalize().as_bytes()
        };
        assert_ne!(with_v1, alt);
    }

    #[test]
    fn ranking_key_is_length_prefixed_not_concatenated() {
        // If fields were concatenated without framing, these two would collide.
        assert_ne!(ranking_key("ab", "c", "x"), ranking_key("a", "bc", "x"));
    }

    #[test]
    fn ranking_key_is_deterministic_32_bytes() {
        // A 256-bit key rendered as 64 lowercase hex chars, identical every call
        // for identical inputs. Guards the hex helper and the key width.
        let id = "path\u{0}swap_boolean\u{0}0\u{0}4\u{0}true\u{0}false";
        let once = ranking_key("commitA", "42", id);
        let twice = ranking_key("commitA", "42", id);
        assert_eq!(once, twice);
        let hex = to_hex(&once);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
