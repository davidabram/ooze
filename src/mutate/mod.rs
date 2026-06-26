use std::collections::BTreeSet;

use crate::core::{MutationCandidate, MutatorImpl, OperatorName};

pub mod registry;

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorFilterMode {
    RegistryDefaults,
    Explicit,
}

#[derive(Debug, Clone)]
pub struct OperatorFilter {
    pub mode: OperatorFilterMode,
    pub include: BTreeSet<OperatorName>,
    pub exclude: BTreeSet<OperatorName>,
}

impl OperatorFilter {
    pub fn from_cli(operators: &[OperatorName], exclude_operators: &[OperatorName]) -> Self {
        let exclude: BTreeSet<OperatorName> = exclude_operators.iter().copied().collect();
        if operators.is_empty() {
            let include = OperatorName::ALL
                .iter()
                .copied()
                .filter(|op| op.info().default_enabled)
                .collect();
            Self {
                mode: OperatorFilterMode::RegistryDefaults,
                include,
                exclude,
            }
        } else {
            Self {
                mode: OperatorFilterMode::Explicit,
                include: operators.iter().copied().collect(),
                exclude,
            }
        }
    }

    /// A filter that admits every implementation, regardless of per-operator or
    /// per-implementation defaults. Used by commands that must reproduce an
    /// arbitrary candidate id (raw discovery, apply-mutant, test-mutant).
    pub fn allow_all() -> Self {
        Self {
            mode: OperatorFilterMode::Explicit,
            include: OperatorName::ALL.iter().copied().collect(),
            exclude: BTreeSet::new(),
        }
    }

    pub fn allows(&self, op: OperatorName) -> bool {
        if self.exclude.contains(&op) {
            return false;
        }
        self.include.contains(&op)
    }

    /// Whether a specific implementation should run. In `RegistryDefaults` mode
    /// the per-implementation default is the authority (so a language can opt out
    /// of an otherwise default-on operator); in `Explicit` mode the user's
    /// `--operators` selection decides, by semantic operator.
    pub fn allows_impl(&self, m: &MutatorImpl) -> bool {
        match self.mode {
            OperatorFilterMode::RegistryDefaults => {
                m.default_enabled() && !self.exclude.contains(&m.operator)
            }
            OperatorFilterMode::Explicit => self.allows(m.operator),
        }
    }

    pub fn included_after_excludes(&self) -> Vec<OperatorName> {
        self.include
            .iter()
            .copied()
            .filter(|op| !self.exclude.contains(op))
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OperatorFilterReport {
    pub mode: OperatorFilterMode,
    pub included: Vec<OperatorName>,
    pub excluded: Vec<OperatorName>,
}

impl From<&OperatorFilter> for OperatorFilterReport {
    fn from(f: &OperatorFilter) -> Self {
        Self {
            mode: f.mode,
            included: f.included_after_excludes(),
            excluded: f.exclude.iter().copied().collect(),
        }
    }
}
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, QueryCursor};

use crate::core::FunctionSpan;
use crate::lang::{CompiledMutator, CompiledRegistry};

/// Discover mutation candidates using queries compiled once for this run. The
/// registry was built with the operator filter already applied, so each compiled
/// language carries exactly the mutators that should run.
///
/// Functions are grouped by source file so each file is read and parsed once,
/// regardless of how many functions it contains, then every mutator runs against
/// each function's node in that single tree.
pub fn discover_mutants(
    functions: &[FunctionSpan],
    registry: &CompiledRegistry,
) -> Result<Vec<MutationCandidate>> {
    // BTreeMap keeps file iteration deterministic; insertion order within a file
    // preserves the scan order of its functions.
    let mut by_file: std::collections::BTreeMap<&Path, Vec<&FunctionSpan>> =
        std::collections::BTreeMap::new();
    for function in functions {
        by_file.entry(&function.file).or_default().push(function);
    }

    let mut candidates = Vec::new();

    for (file, spans) in by_file {
        // All spans in a file share its language.
        let language = spans[0].language;
        let Some(compiled) = registry.for_language(language) else {
            continue;
        };
        if compiled.mutators.is_empty() {
            continue;
        }

        let source = std::fs::read_to_string(file)
            .with_context(|| format!("reading {}", file.display()))?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&compiled.ts_language)
            .with_context(|| format!("loading {language} grammar"))?;

        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("parsing {}", file.display()))?;

        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        for function in spans {
            let Some(function_node) =
                find_node_by_byte_range(root, function.start_byte, function.end_byte)
            else {
                continue;
            };

            for m in &compiled.mutators {
                run_mutator(m, function, function_node, source_bytes, &mut candidates);
            }
        }
    }

    let mut candidates = dedupe_overlapping(candidates);
    // Deterministic output independent of file grouping / mutator order, so
    // `mutants` and `plan-mutants` are stable across runs.
    candidates.sort_by(|a, b| {
        (a.file.as_path(), a.line, a.column, a.implementation.as_str()).cmp(&(
            b.file.as_path(),
            b.line,
            b.column,
            b.implementation.as_str(),
        ))
    });
    Ok(candidates)
}

/// Run one compiled mutator against a single function node, pushing a candidate
/// for every `target` capture whose replacement applies.
fn run_mutator(
    m: &CompiledMutator,
    function: &FunctionSpan,
    function_node: Node,
    source_bytes: &[u8],
    candidates: &mut Vec<MutationCandidate>,
) {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&m.query, function_node, source_bytes);

    while let Some(captured) = matches.next() {
        for capture in captured.captures {
            if capture.index != m.target_index {
                continue;
            }

            let node = capture.node;
            let mut original = node_text(node, source_bytes);
            let Some(replacement) = (m.spec.replacement)(&original) else {
                continue;
            };

            // A deletion (empty replacement) that removes a whole node would
            // otherwise leave the separator that preceded it, e.g.
            // `[x for x in xs if p]` -> `[x for x in xs ]`. Absorb one preceding
            // space into the range so the edit reads cleanly.
            let mut start_byte = node.start_byte();
            if replacement.is_empty() && start_byte > 0 && source_bytes[start_byte - 1] == b' ' {
                start_byte -= 1;
                original.insert(0, ' ');
            }

            let candidate_file = normalize_path(&function.file);

            candidates.push(MutationCandidate {
                id: format!(
                    "{}:{}:{}:{}",
                    candidate_file.display(),
                    node.start_position().row + 1,
                    node.start_position().column,
                    m.spec.id,
                ),
                file: function.file.clone(),
                language: function.language,
                function: function.name.clone(),
                line: node.start_position().row + 1,
                column: node.start_position().column,
                start_byte,
                end_byte: node.end_byte(),
                operator: m.spec.operator,
                operator_category: m.spec.category(),
                implementation: m.spec.id.to_string(),
                description: (m.spec.description)(&original, &replacement),
                original,
                replacement,
            });
        }
    }
}

/// Drop redundant candidates that rewrite the same source bytes to the same
/// text. Two operators can match overlapping nodes yet produce a byte-identical
/// mutant: `swap_boolean` and `return_boolean` both turn `return true` into
/// `return false`, and `negate_equality` (rewriting `==`) and `len_zero_boundary`
/// (rewriting the whole `len(x) == 0`) both yield `len(x) != 0`. Building and
/// testing both wastes work and double-counts the site, so we keep one per
/// distinct edit — the most specific operator (see `OperatorName::dedup_priority`).
///
/// Edits are compared by their *canonical* form: each `(start_byte, original,
/// replacement)` is reduced to the minimal changed span by stripping the common
/// prefix and suffix. This is what lets the two `len(x) == 0` edits — which span
/// different byte ranges — collapse to the same `=`→`!` change.
fn dedupe_overlapping(candidates: Vec<MutationCandidate>) -> Vec<MutationCandidate> {
    use std::collections::HashMap;

    let mut slot_for_key: HashMap<(PathBuf, usize, usize, String), usize> = HashMap::new();
    let mut kept: Vec<MutationCandidate> = Vec::with_capacity(candidates.len());

    for cand in candidates {
        let (start, end, repl) =
            canonical_edit(cand.start_byte, &cand.original, &cand.replacement);
        let key = (cand.file.clone(), start, end, repl);

        if let Some(&idx) = slot_for_key.get(&key) {
            if cand.operator.dedup_priority() > kept[idx].operator.dedup_priority() {
                kept[idx] = cand;
            }
        } else {
            slot_for_key.insert(key, kept.len());
            kept.push(cand);
        }
    }

    kept
}

/// Reduce an edit (replace `original` at `start_byte` with `replacement`) to the
/// minimal byte range that actually changes, by stripping the common prefix and
/// suffix shared by `original` and `replacement`. Returns the canonical
/// `(start_byte, end_byte, replacement_text)`. Affixes are trimmed on char
/// boundaries so the byte offsets stay valid for non-ASCII source.
fn canonical_edit(start_byte: usize, original: &str, replacement: &str) -> (usize, usize, String) {
    let prefix = common_prefix_len(original, replacement);
    let suffix = common_suffix_len(&original[prefix..], &replacement[prefix..]);
    let canon_start = start_byte + prefix;
    let canon_end = start_byte + original.len() - suffix;
    let canon_repl = replacement[prefix..replacement.len() - suffix].to_string();
    (canon_start, canon_end, canon_repl)
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut len = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        len += ca.len_utf8();
    }
    len
}

fn common_suffix_len(a: &str, b: &str) -> usize {
    let mut len = 0;
    let mut ai = a.chars().rev();
    let mut bi = b.chars().rev();
    while let (Some(ca), Some(cb)) = (ai.next(), bi.next()) {
        if ca != cb {
            break;
        }
        len += ca.len_utf8();
    }
    len
}

fn find_node_by_byte_range(root: Node, start_byte: usize, end_byte: usize) -> Option<Node> {
    fn visit(node: Node, start_byte: usize, end_byte: usize) -> Option<Node> {
        if node.start_byte() == start_byte && node.end_byte() == end_byte {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.start_byte() <= start_byte
                && child.end_byte() >= end_byte
                && let Some(found) = visit(child, start_byte, end_byte)
            {
                return Some(found);
            }
        }

        None
    }

    visit(root, start_byte, end_byte)
}

fn normalize_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if let Some(stripped) = path_str.strip_prefix("./") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn node_text(node: Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("<invalid utf8>")
        .to_string()
}

#[cfg(test)]
mod canonical_edit_tests {
    use super::canonical_edit;

    #[test]
    fn shared_affixes_are_trimmed() {
        // `swap_boolean` on a plain `true` literal at byte 10. The shared trailing
        // `e` is stripped, leaving the minimal `tru`->`fals` edit.
        assert_eq!(canonical_edit(10, "true", "false"), (10, 13, "fals".into()));
    }

    #[test]
    fn whole_expression_and_operator_edits_collapse() {
        // The two `len(x) == 0` mutants: `negate_equality` rewrites just the `==`,
        // `len_zero_boundary` rewrites the whole comparison. Both must canonicalize
        // to the same minimal `=`->`!` edit at the same absolute byte so they dedupe.
        let whole = canonical_edit(5, "len(x) == 0", "len(x) != 0");
        // `==` sits 7 bytes into the comparison (after `len(x) `), so at byte 12.
        let operator = canonical_edit(12, "==", "!=");
        assert_eq!(whole, operator);
        assert_eq!(whole, (12, 13, "!".into()));
    }
}

#[cfg(test)]
mod discover_tests {
    use super::*;
    use crate::core::Language;
    use std::path::Path;

    #[test]
    fn discover_sets_language_qualified_implementation_and_id() {
        let functions = crate::lang::scan_directory(Path::new("tests/fixtures/mutate"))
            .expect("scanning fixtures");
        let registry =
            CompiledRegistry::compile(crate::lang::supported_languages(), &OperatorFilter::allow_all())
                .unwrap();
        let candidates = discover_mutants(&functions, &registry).unwrap();

        assert!(!candidates.is_empty(), "fixture should yield candidates");
        for c in &candidates {
            assert_eq!(c.language, Language::Rust);
            assert_eq!(c.implementation, format!("rust.{}", c.operator.as_str()));
            assert_eq!(c.operator_category, c.operator.info().category);
            assert!(
                c.id.ends_with(&c.implementation),
                "id {:?} should end with implementation {:?}",
                c.id,
                c.implementation
            );
        }
    }

    #[test]
    fn discovery_output_is_deterministically_ordered() {
        // Grouping by file plus the final sort must make the candidate vector
        // (order included) identical across runs, and sorted by source position.
        let functions = crate::lang::scan_directory(Path::new("tests/fixtures/mutate"))
            .expect("scanning fixtures");
        let registry = CompiledRegistry::compile(
            crate::lang::supported_languages(),
            &OperatorFilter::allow_all(),
        )
        .unwrap();

        let first = discover_mutants(&functions, &registry).unwrap();
        let second = discover_mutants(&functions, &registry).unwrap();
        let ids_first: Vec<&str> = first.iter().map(|c| c.id.as_str()).collect();
        let ids_second: Vec<&str> = second.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids_first, ids_second, "discovery order must be stable");

        let sorted = {
            let mut s = first.clone();
            s.sort_by(|a, b| {
                (a.file.as_path(), a.line, a.column, a.implementation.as_str()).cmp(&(
                    b.file.as_path(),
                    b.line,
                    b.column,
                    b.implementation.as_str(),
                ))
            });
            s
        };
        assert_eq!(
            ids_first,
            sorted.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            "candidates must come out sorted by (file, line, column, implementation)"
        );
    }
}

#[cfg(test)]
mod operator_fixture_tests {
    use super::*;
    use crate::core::{Language, OperatorName};
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Compact, location-free shape of a discovered candidate. Dropping the
    /// line/column keeps these assertions stable when a fixture is edited, while
    /// still pinning down which operator fired and the exact text rewrite. Adding
    /// a new language is then just: drop a fixture, list its expected mutants.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct ExpectedMutant {
        language: Language,
        function: String,
        operator: OperatorName,
        original: String,
        replacement: String,
    }

    fn expect(
        language: Language,
        function: &str,
        operator: OperatorName,
        original: &str,
        replacement: &str,
    ) -> ExpectedMutant {
        ExpectedMutant {
            language,
            function: function.to_string(),
            operator,
            original: original.to_string(),
            replacement: replacement.to_string(),
        }
    }

    /// Discover every candidate under a fixture directory (all operators enabled,
    /// so default-disabled ones like `integer_zero_one` are included) and collapse
    /// them to the compact shape. Duplicate shapes at different locations dedupe.
    fn discovered(dir: &str) -> BTreeSet<ExpectedMutant> {
        let functions = crate::lang::scan_directory(Path::new(dir)).expect("scanning fixture");
        let registry =
            CompiledRegistry::compile(crate::lang::supported_languages(), &OperatorFilter::allow_all())
                .unwrap();
        let candidates = discover_mutants(&functions, &registry).unwrap();
        candidates
            .iter()
            .map(|c| ExpectedMutant {
                language: c.language,
                function: c.function.clone(),
                operator: c.operator,
                original: c.original.clone(),
                replacement: c.replacement.clone(),
            })
            .collect()
    }

    /// Like `discovered`, but restricted to an explicit set of operators. Used to
    /// exercise the default-disabled advanced operators in isolation, the way the
    /// `--operators` selection would.
    fn discovered_ops(dir: &str, ops: &[OperatorName]) -> BTreeSet<ExpectedMutant> {
        let functions = crate::lang::scan_directory(Path::new(dir)).expect("scanning fixture");
        let filter = OperatorFilter::from_cli(ops, &[]);
        let registry =
            CompiledRegistry::compile(crate::lang::supported_languages(), &filter).unwrap();
        let candidates = discover_mutants(&functions, &registry).unwrap();
        candidates
            .iter()
            .map(|c| ExpectedMutant {
                language: c.language,
                function: c.function.clone(),
                operator: c.operator,
                original: c.original.clone(),
                replacement: c.replacement.clone(),
            })
            .collect()
    }

    const ADVANCED_JS_TS_OPERATORS: &[OperatorName] = &[
        OperatorName::NullishCoalescingRemoval,
        OperatorName::OptionalChainingRemoval,
        OperatorName::TernaryArmSwap,
        OperatorName::ArrayEmptyLiteral,
        OperatorName::ObjectEmptyLiteral,
        OperatorName::StringEmptyLiteral,
        OperatorName::AwaitRemoval,
    ];

    #[test]
    fn rust_operator_fixture_discovers_expected_mutants() {
        use Language::Rust;
        use OperatorName::{
            ComparisonBoundary, ComparisonNegation, EmptyVecMacro, ExpectToUnwrapOrDefault,
            IntegerZeroOne, IteratorAnyAll, MatchBoolPattern, MatchWildcardToPanic, MinMaxSwap,
            NegateEquality, NegatePredicateMethod, OkErrBoolean, OptionSomeNone,
            RangeInclusiveExclusive, RemoveNot, RemoveTry, ReturnBoolean, SaturatingCheckedSwap,
            SomeBoolean, SwapBoolean, SwapLogical, SwapPredicateMethod, UnwrapToUnwrapOrDefault,
        };

        let got = discovered("tests/fixtures/operators/rust/sample.rs");
        let want: BTreeSet<ExpectedMutant> = [
            expect(Rust, "swap_boolean", SwapBoolean, "true", "false"),
            expect(Rust, "negate_equality", NegateEquality, "==", "!="),
            // `compare`'s single `<` drives both comparison operators.
            expect(Rust, "compare", ComparisonBoundary, "<", "<="),
            expect(Rust, "compare", ComparisonNegation, "<", ">="),
            expect(Rust, "swap_logical", SwapLogical, "&&", "||"),
            expect(Rust, "remove_not", RemoveNot, "!flag", "flag"),
            expect(Rust, "integer_zero_one", IntegerZeroOne, "0", "1"),
            // `range_bound` has a `0` literal and the `0..n` range bound.
            expect(Rust, "range_bound", IntegerZeroOne, "0", "1"),
            expect(Rust, "range_bound", RangeInclusiveExclusive, "..", "..="),
            expect(
                Rust,
                "swap_predicate_method",
                SwapPredicateMethod,
                "is_some",
                "is_none",
            ),
            expect(
                Rust,
                "negate_predicate_method",
                NegatePredicateMethod,
                "s.is_empty()",
                "!s.is_empty()",
            ),
            // `return true` is matched by both return_boolean and swap_boolean; the
            // identical mutant is deduped down to the more specific return_boolean.
            expect(Rust, "return_boolean", ReturnBoolean, "true", "false"),
            expect(Rust, "iterator_any_all", IteratorAnyAll, "any", "all"),
            // Each match-arm boolean pattern is matched by both match_bool_pattern
            // and swap_boolean; the identical mutants dedupe to match_bool_pattern.
            expect(Rust, "match_bool_pattern", MatchBoolPattern, "true", "false"),
            expect(Rust, "match_bool_pattern", MatchBoolPattern, "false", "true"),
            // `Ok(true)`: ok_err_boolean and swap_boolean coincide; keep ok_err_boolean.
            expect(Rust, "ok_err_boolean", OkErrBoolean, "true", "false"),
            // `Some(true)`: some_boolean and swap_boolean coincide on the literal
            // (kept as some_boolean), plus option_some_none on the whole call.
            expect(Rust, "some_boolean", SomeBoolean, "true", "false"),
            expect(Rust, "some_boolean", OptionSomeNone, "Some(true)", "None"),
            expect(Rust, "option_some_none", OptionSomeNone, "Some(x)", "None"),
            expect(Rust, "remove_try", RemoveTry, "r?", "r"),
            expect(
                Rust,
                "unwrap_to_unwrap_or_default",
                UnwrapToUnwrapOrDefault,
                "unwrap",
                "unwrap_or_default",
            ),
            expect(Rust, "min_max_swap", MinMaxSwap, "min", "max"),
            expect(
                Rust,
                "match_wildcard_to_panic",
                MatchWildcardToPanic,
                "200",
                "panic!(\"ooze mutant\")",
            ),
            expect(Rust, "empty_vec_macro", EmptyVecMacro, "vec![3, 4, 5]", "vec![]"),
            expect(
                Rust,
                "saturating_checked_swap",
                SaturatingCheckedSwap,
                "checked_add",
                "saturating_add",
            ),
            expect(
                Rust,
                "expect_to_unwrap_or_default",
                ExpectToUnwrapOrDefault,
                "opt.expect(\"must be present\")",
                "opt.unwrap_or_default()",
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn javascript_operator_fixture_discovers_expected_mutants() {
        use Language::JavaScript;
        use OperatorName::{
            ComparisonBoundary, ComparisonNegation, NegateEquality, RemoveNot, SwapBoolean,
            SwapLogical,
        };

        let got = discovered("tests/fixtures/operators/javascript/sample.js");
        let want: BTreeSet<ExpectedMutant> = [
            expect(JavaScript, "swapBoolean", SwapBoolean, "true", "false"),
            expect(JavaScript, "negateEquality", NegateEquality, "==", "!="),
            // `compare`'s single `<` drives both comparison operators.
            expect(JavaScript, "compare", ComparisonBoundary, "<", "<="),
            expect(JavaScript, "compare", ComparisonNegation, "<", ">="),
            expect(JavaScript, "swapLogical", SwapLogical, "&&", "||"),
            expect(JavaScript, "removeNot", RemoveNot, "!flag", "flag"),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn python_operator_fixture_discovers_expected_mutants() {
        use Language::Python;
        use OperatorName::{
            ComparisonBoundary, ComparisonNegation, IntegerZeroOne, IteratorAnyAll, MinMaxSwap,
            NegateEquality, NegatePredicateMethod, NoneReturn, ReturnBoolean, SwapBoolean,
            SwapLogical, TruthinessNegation,
        };

        let got = discovered("tests/fixtures/operators/python/sample.py");
        let want: BTreeSet<ExpectedMutant> = [
            expect(Python, "swap_boolean", SwapBoolean, "True", "False"),
            expect(Python, "negate_equality", NegateEquality, "==", "!="),
            // `compare`'s single `<` drives both comparison operators.
            expect(Python, "compare", ComparisonBoundary, "<", "<="),
            expect(Python, "compare", ComparisonNegation, "<", ">="),
            expect(Python, "swap_logical", SwapLogical, "and", "or"),
            expect(Python, "integer_zero_one", IntegerZeroOne, "0", "1"),
            // Every function returns a non-None value, so none_return also fires.
            expect(Python, "swap_boolean", NoneReturn, "enabled", "None"),
            expect(Python, "negate_equality", NoneReturn, "a == b", "None"),
            expect(Python, "compare", NoneReturn, "a < b", "None"),
            expect(Python, "swap_logical", NoneReturn, "x and y", "None"),
            expect(Python, "integer_zero_one", NoneReturn, "n", "None"),
            // `any(...)` drives iterator_any_all; the returned call also feeds none_return.
            expect(Python, "quantifier", IteratorAnyAll, "any", "all"),
            expect(
                Python,
                "quantifier",
                NoneReturn,
                "any(x.active for x in items)",
                "None",
            ),
            // Each returned boolean is matched by return_boolean and swap_boolean;
            // the identical flip dedupes to return_boolean. Both literals also feed
            // none_return (a distinct mutant), and `if flag:` feeds truthiness.
            expect(Python, "returns_boolean", ReturnBoolean, "True", "False"),
            expect(Python, "returns_boolean", ReturnBoolean, "False", "True"),
            expect(Python, "returns_boolean", NoneReturn, "True", "None"),
            expect(Python, "returns_boolean", NoneReturn, "False", "None"),
            expect(Python, "returns_boolean", TruthinessNegation, "flag", "not (flag)"),
            // `value.isdigit()` drives negate_predicate_method; the call also feeds none_return.
            expect(
                Python,
                "string_predicate",
                NegatePredicateMethod,
                "value.isdigit()",
                "not (value.isdigit())",
            ),
            expect(
                Python,
                "string_predicate",
                NoneReturn,
                "value.isdigit()",
                "None",
            ),
            // `min`/`max` drive min_max_swap (default-disabled); the returned tuple feeds none_return.
            expect(Python, "bounds", MinMaxSwap, "min", "max"),
            expect(Python, "bounds", MinMaxSwap, "max", "min"),
            expect(
                Python,
                "bounds",
                NoneReturn,
                "min(values), max(values)",
                "None",
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn python_specific_operator_fixture_discovers_expected_mutants() {
        use Language::Python;
        use OperatorName::{
            ComprehensionFilterRemoval, DictGetDefaultRemoval, EmptyCollectionLiteral,
            InNegation, IntegerZeroOne, IsNoneNegation, LenZeroBoundary, NoneReturn,
            TruthinessNegation,
        };

        let got = discovered("tests/fixtures/operators/python_specific");
        let want: BTreeSet<ExpectedMutant> = [
            expect(Python, "is_none", IsNoneNegation, "is", "is not"),
            expect(Python, "membership", InNegation, "in", "not in"),
            expect(Python, "truthiness", TruthinessNegation, "x", "not (x)"),
            // `len(xs) == 0`: len_zero_boundary (whole comparison) and negate_equality
            // (`==` token) produce the byte-identical `len(xs) != 0`, so the mutant
            // dedupes to the more specific len_zero_boundary. The `0` still feeds
            // integer_zero_one (a distinct mutant).
            expect(
                Python,
                "len_boundary",
                LenZeroBoundary,
                "len(xs) == 0",
                "len(xs) != 0",
            ),
            expect(Python, "len_boundary", IntegerZeroOne, "0", "1"),
            // `d.get(k, 0)` drops its default; the `0` also feeds integer_zero_one.
            expect(
                Python,
                "dict_default",
                DictGetDefaultRemoval,
                "d.get(k, 0)",
                "d.get(k)",
            ),
            expect(Python, "dict_default", IntegerZeroOne, "0", "1"),
            // The leading space is absorbed so removal reads cleanly.
            expect(
                Python,
                "comprehension",
                ComprehensionFilterRemoval,
                " if keep(x)",
                "",
            ),
            expect(Python, "none_return", NoneReturn, "value", "None"),
            expect(
                Python,
                "empty_list",
                EmptyCollectionLiteral,
                "[a, b]",
                "[]",
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn typescript_operator_fixture_discovers_expected_mutants() {
        use Language::TypeScript;
        use OperatorName::{
            ComparisonBoundary, ComparisonNegation, NegateEquality, RemoveNot, SwapBoolean,
            SwapLogical,
        };

        let got = discovered("tests/fixtures/operators/typescript/sample.ts");
        let want: BTreeSet<ExpectedMutant> = [
            expect(TypeScript, "swapBoolean", SwapBoolean, "true", "false"),
            expect(TypeScript, "negateEquality", NegateEquality, "==", "!="),
            // `compare`'s single `<` drives both comparison operators.
            expect(TypeScript, "compare", ComparisonBoundary, "<", "<="),
            expect(TypeScript, "compare", ComparisonNegation, "<", ">="),
            expect(TypeScript, "swapLogical", SwapLogical, "&&", "||"),
            expect(TypeScript, "removeNot", RemoveNot, "!flag", "flag"),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn javascript_advanced_operator_fixture_discovers_expected_mutants() {
        use Language::JavaScript;
        use OperatorName::{
            ArrayEmptyLiteral, AwaitRemoval, NullishCoalescingRemoval, ObjectEmptyLiteral,
            OptionalChainingRemoval, StringEmptyLiteral, TernaryArmSwap,
        };

        let got = discovered_ops(
            "tests/fixtures/operators/javascript/all.js",
            ADVANCED_JS_TS_OPERATORS,
        );
        let want: BTreeSet<ExpectedMutant> = [
            expect(
                JavaScript,
                "fallback",
                NullishCoalescingRemoval,
                "value ?? fallbackValue",
                "value",
            ),
            expect(
                JavaScript,
                "optionalAccess",
                OptionalChainingRemoval,
                "user?.name",
                "user.name",
            ),
            expect(JavaScript, "optionalCall", OptionalChainingRemoval, "fn?.()", "fn()"),
            expect(JavaScript, "choose", TernaryArmSwap, "flag ? a : b", "flag ? b : a"),
            expect(JavaScript, "arrayLiteral", ArrayEmptyLiteral, "[1, 2, 3]", "[]"),
            expect(JavaScript, "objectLiteral", ObjectEmptyLiteral, "{ a: 1, b: 2 }", "{}"),
            expect(JavaScript, "stringLiteral", StringEmptyLiteral, "\"hello\"", "\"\""),
            // `boundary`'s prefix/suffix string arguments are also non-empty literals.
            expect(JavaScript, "boundary", StringEmptyLiteral, "\"pre\"", "\"\""),
            expect(JavaScript, "boundary", StringEmptyLiteral, "\".txt\"", "\"\""),
            expect(JavaScript, "awaitValue", AwaitRemoval, "await promise", "promise"),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }

    #[test]
    fn typescript_advanced_operator_fixture_discovers_expected_mutants() {
        use Language::TypeScript;
        use OperatorName::{
            ArrayEmptyLiteral, AwaitRemoval, NullishCoalescingRemoval, ObjectEmptyLiteral,
            OptionalChainingRemoval, StringEmptyLiteral, TernaryArmSwap,
        };

        let got = discovered_ops(
            "tests/fixtures/operators/typescript/all.ts",
            ADVANCED_JS_TS_OPERATORS,
        );
        let want: BTreeSet<ExpectedMutant> = [
            expect(
                TypeScript,
                "fallback",
                NullishCoalescingRemoval,
                "value ?? fallbackValue",
                "value",
            ),
            expect(
                TypeScript,
                "optionalAccess",
                OptionalChainingRemoval,
                "user?.name",
                "user.name",
            ),
            expect(TypeScript, "optionalCall", OptionalChainingRemoval, "fn?.()", "fn()"),
            expect(TypeScript, "choose", TernaryArmSwap, "flag ? a : b", "flag ? b : a"),
            expect(TypeScript, "arrayLiteral", ArrayEmptyLiteral, "[1, 2, 3]", "[]"),
            expect(TypeScript, "objectLiteral", ObjectEmptyLiteral, "{ a: 1, b: 2 }", "{}"),
            expect(TypeScript, "stringLiteral", StringEmptyLiteral, "\"hello\"", "\"\""),
            // `boundary`'s prefix/suffix string arguments are also non-empty literals.
            expect(TypeScript, "boundary", StringEmptyLiteral, "\"pre\"", "\"\""),
            expect(TypeScript, "boundary", StringEmptyLiteral, "\".txt\"", "\"\""),
            expect(TypeScript, "awaitValue", AwaitRemoval, "await promise", "promise"),
        ]
        .into_iter()
        .collect();

        assert_eq!(got, want);
    }
}
