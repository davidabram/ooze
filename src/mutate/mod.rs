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
use tree_sitter::{Node, Query, QueryCursor};

use crate::lang::GrammarDef;

pub fn discover_mutants(
    functions: &[crate::core::FunctionSpan],
    grammars: &[&GrammarDef],
    filter: &OperatorFilter,
) -> Result<Vec<MutationCandidate>> {
    let mut candidates = Vec::new();

    for function in functions {
        let Some(grammar) = grammars.iter().find(|g| g.id == function.language) else {
            continue;
        };

        let impls: Vec<&MutatorImpl> = registry::implementations_for_language(function.language)
            .filter(|m| filter.allows_impl(m))
            .collect();
        if impls.is_empty() {
            continue;
        }

        let source = std::fs::read_to_string(&function.file)
            .with_context(|| format!("reading {}", function.file.display()))?;

        let ts_lang = (grammar.language)();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts_lang)
            .with_context(|| format!("loading {} grammar", function.language))?;

        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("parsing {}", function.file.display()))?;

        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let function_node = find_node_by_byte_range(root, function.start_byte, function.end_byte);

        let Some(function_node) = function_node else {
            continue;
        };

        for m in impls {
            let query = Query::new(&ts_lang, m.query)
                .with_context(|| format!("compiling {} mutation query", m.id))?;

            let Some(target_idx) = query.capture_index_for_name("target") else {
                continue;
            };

            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, function_node, source_bytes);

            while let Some(captured) = matches.next() {
                for capture in captured.captures {
                    if capture.index != target_idx {
                        continue;
                    }

                    let node = capture.node;
                    let mut original = node_text(node, source_bytes);
                    let Some(replacement) = (m.replacement)(&original) else {
                        continue;
                    };

                    // A deletion (empty replacement) that removes a whole node
                    // would otherwise leave the separator that preceded it, e.g.
                    // `[x for x in xs if p]` -> `[x for x in xs ]`. Absorb one
                    // preceding space into the range so the edit reads cleanly.
                    let mut start_byte = node.start_byte();
                    if replacement.is_empty()
                        && start_byte > 0
                        && source_bytes[start_byte - 1] == b' '
                    {
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
                            m.id,
                        ),
                        file: function.file.clone(),
                        language: function.language,
                        function: function.name.clone(),
                        line: node.start_position().row + 1,
                        column: node.start_position().column,
                        start_byte,
                        end_byte: node.end_byte(),
                        operator: m.operator,
                        operator_category: m.category(),
                        implementation: m.id.to_string(),
                        description: (m.description)(&original, &replacement),
                        original,
                        replacement,
                    });
                }
            }
        }
    }

    Ok(candidates)
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
mod discover_tests {
    use super::*;
    use crate::core::Language;
    use std::path::Path;

    #[test]
    fn discover_sets_language_qualified_implementation_and_id() {
        let functions = crate::lang::scan_directory(Path::new("tests/fixtures/mutate"))
            .expect("scanning fixtures");
        let grammars = crate::lang::supported_languages();
        let candidates =
            discover_mutants(&functions, grammars, &OperatorFilter::allow_all()).unwrap();

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
        let grammars = crate::lang::supported_languages();
        let candidates =
            discover_mutants(&functions, grammars, &OperatorFilter::allow_all()).unwrap();
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

    #[test]
    fn rust_operator_fixture_discovers_expected_mutants() {
        use Language::Rust;
        use OperatorName::{
            ComparisonBoundary, ComparisonNegation, IntegerZeroOne, NegateEquality,
            NegatePredicateMethod, RangeInclusiveExclusive, RemoveNot, ReturnBoolean, SwapBoolean,
            SwapLogical, SwapPredicateMethod,
        };

        let got = discovered("tests/fixtures/operators/rust");
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
            // The literal in `return true` drives both return_boolean and swap_boolean.
            expect(Rust, "return_boolean", ReturnBoolean, "true", "false"),
            expect(Rust, "return_boolean", SwapBoolean, "true", "false"),
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

        let got = discovered("tests/fixtures/operators/javascript");
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
            ComparisonBoundary, ComparisonNegation, IntegerZeroOne, NegateEquality, NoneReturn,
            SwapBoolean, SwapLogical,
        };

        let got = discovered("tests/fixtures/operators/python");
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
            InNegation, IntegerZeroOne, IsNoneNegation, LenZeroBoundary, NegateEquality,
            NoneReturn, TruthinessNegation,
        };

        let got = discovered("tests/fixtures/operators/python_specific");
        let want: BTreeSet<ExpectedMutant> = [
            expect(Python, "is_none", IsNoneNegation, "is", "is not"),
            expect(Python, "membership", InNegation, "in", "not in"),
            expect(Python, "truthiness", TruthinessNegation, "x", "not (x)"),
            // `len(xs) == 0` drives three operators on overlapping nodes.
            expect(
                Python,
                "len_boundary",
                LenZeroBoundary,
                "len(xs) == 0",
                "len(xs) != 0",
            ),
            expect(Python, "len_boundary", NegateEquality, "==", "!="),
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

        let got = discovered("tests/fixtures/operators/typescript");
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
}
