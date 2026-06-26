use std::collections::BTreeMap;
use std::path::PathBuf;

/// A language ooze can parse and mutate. Serializes to the same canonical string
/// the grammar previously returned from `name()` (e.g. `javascript`, `c_sharp`),
/// so report/JSON consumers are unaffected by the move to a typed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    Bash,
    C,
    Cpp,
    CSharp,
    Dart,
    Elixir,
    Erlang,
    Gleam,
    Go,
    Haskell,
    Java,
    JavaScript,
    Julia,
    Lua,
    Ocaml,
    Php,
    Python,
    Ruby,
    Rust,
    Scala,
    Swift,
    TypeScript,
    Zig,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Language::Bash => "bash",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "c_sharp",
            Language::Dart => "dart",
            Language::Elixir => "elixir",
            Language::Erlang => "erlang",
            Language::Gleam => "gleam",
            Language::Go => "go",
            Language::Haskell => "haskell",
            Language::Java => "java",
            Language::JavaScript => "javascript",
            Language::Julia => "julia",
            Language::Lua => "lua",
            Language::Ocaml => "ocaml",
            Language::Php => "php",
            Language::Python => "python",
            Language::Ruby => "ruby",
            Language::Rust => "rust",
            Language::Scala => "scala",
            Language::Swift => "swift",
            Language::TypeScript => "typescript",
            Language::Zig => "zig",
        }
    }

    pub fn parse(s: &str) -> Option<Language> {
        Some(match s {
            "bash" => Language::Bash,
            "c" => Language::C,
            "cpp" => Language::Cpp,
            "c_sharp" => Language::CSharp,
            "dart" => Language::Dart,
            "elixir" => Language::Elixir,
            "erlang" => Language::Erlang,
            "gleam" => Language::Gleam,
            "go" => Language::Go,
            "haskell" => Language::Haskell,
            "java" => Language::Java,
            "javascript" => Language::JavaScript,
            "julia" => Language::Julia,
            "lua" => Language::Lua,
            "ocaml" => Language::Ocaml,
            "php" => Language::Php,
            "python" => Language::Python,
            "ruby" => Language::Ruby,
            "rust" => Language::Rust,
            "scala" => Language::Scala,
            "swift" => Language::Swift,
            "typescript" => Language::TypeScript,
            "zig" => Language::Zig,
            _ => return None,
        })
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for Language {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Language {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Language::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown language {s:?}")))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutantStatus {
    Killed,
    Survived,
    Timeout,
    Error,
}

impl std::fmt::Display for MutantStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutantStatus::Killed => write!(f, "killed"),
            MutantStatus::Survived => write!(f, "survived"),
            MutantStatus::Timeout => write!(f, "timeout"),
            MutantStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutationRunReport {
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub error: usize,
    pub outcomes: Vec<MutantOutcome>,
}

impl MutationRunReport {
    pub fn from_outcomes(outcomes: Vec<MutantOutcome>) -> Self {
        let total = outcomes.len();

        let killed = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Killed))
            .count();

        let survived = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Survived))
            .count();

        let timeout = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Timeout))
            .count();

        let error = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Error))
            .count();

        Self {
            total,
            killed,
            survived,
            timeout,
            error,
            outcomes,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutantOutcome {
    pub candidate: MutationCandidate,
    pub status: MutantStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub diff: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppliedMutation {
    pub candidate: MutationCandidate,
    pub workspace_file: PathBuf,
    pub diff: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FunctionSpan {
    pub file: PathBuf,
    pub language: Language,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub cyclomatic: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CrapEntry {
    pub file: PathBuf,
    pub language: Language,
    pub function: String,
    pub line: usize,
    pub cyclomatic: usize,
    pub coverage: f64,
    pub crap: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FileCoverage {
    pub lines: BTreeMap<u32, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorName {
    SwapBoolean,
    NegateEquality,
    ComparisonBoundary,
    ComparisonNegation,
    SwapLogical,
    RemoveNot,
    IntegerZeroOne,
    RangeInclusiveExclusive,
    SwapPredicateMethod,
    NegatePredicateMethod,
    ReturnBoolean,
    // Python-specific operators.
    IsNoneNegation,
    InNegation,
    TruthinessNegation,
    LenZeroBoundary,
    DictGetDefaultRemoval,
    ComprehensionFilterRemoval,
    NoneReturn,
    EmptyCollectionLiteral,
    // Rust-specific operators.
    IteratorAnyAll,
    MatchBoolPattern,
    OkErrBoolean,
    SomeBoolean,
    OptionSomeNone,
    RemoveTry,
    UnwrapToUnwrapOrDefault,
    MinMaxSwap,
    MatchWildcardToPanic,
    EmptyVecMacro,
    SaturatingCheckedSwap,
    ExpectToUnwrapOrDefault,
    // Cross-language method/collection operators.
    StringBoundaryMethodSwap,
    IncludesNegation,
    SortedReverseFlip,
    DictGetToIndex,
    // JS/TS advanced operators.
    NullishCoalescingRemoval,
    OptionalChainingRemoval,
    TernaryArmSwap,
    ArrayEmptyLiteral,
    ObjectEmptyLiteral,
    StringEmptyLiteral,
    AwaitRemoval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorCategory {
    BooleanLiteral,
    Equality,
    Comparison,
    Logical,
    NumericLiteral,
    Arithmetic,
    Assignment,
    Conditional,
    Statement,
    Collection,
    Object,
    String,
    Regex,
    Method,
    Nullability,
    Membership,
    Truthiness,
    CollectionBoundary,
    Comprehension,
    Dict,
    ErrorHandling,
    PatternMatching,
    RangeBoundary,
    ReturnValue,
}

impl OperatorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            OperatorCategory::BooleanLiteral => "boolean_literal",
            OperatorCategory::Equality => "equality",
            OperatorCategory::Comparison => "comparison",
            OperatorCategory::Logical => "logical",
            OperatorCategory::NumericLiteral => "numeric_literal",
            OperatorCategory::Arithmetic => "arithmetic",
            OperatorCategory::Assignment => "assignment",
            OperatorCategory::Conditional => "conditional",
            OperatorCategory::Statement => "statement",
            OperatorCategory::Collection => "collection",
            OperatorCategory::Object => "object",
            OperatorCategory::String => "string",
            OperatorCategory::Regex => "regex",
            OperatorCategory::Method => "method",
            OperatorCategory::Nullability => "nullability",
            OperatorCategory::Membership => "membership",
            OperatorCategory::Truthiness => "truthiness",
            OperatorCategory::CollectionBoundary => "collection_boundary",
            OperatorCategory::Comprehension => "comprehension",
            OperatorCategory::Dict => "dict",
            OperatorCategory::ErrorHandling => "error_handling",
            OperatorCategory::PatternMatching => "pattern_matching",
            OperatorCategory::RangeBoundary => "range_boundary",
            OperatorCategory::ReturnValue => "return_value",
        }
    }

    pub fn parse(s: &str) -> Option<OperatorCategory> {
        match s {
            "boolean_literal" => Some(OperatorCategory::BooleanLiteral),
            "equality" => Some(OperatorCategory::Equality),
            "comparison" => Some(OperatorCategory::Comparison),
            "logical" => Some(OperatorCategory::Logical),
            "numeric_literal" => Some(OperatorCategory::NumericLiteral),
            "arithmetic" => Some(OperatorCategory::Arithmetic),
            "assignment" => Some(OperatorCategory::Assignment),
            "conditional" => Some(OperatorCategory::Conditional),
            "statement" => Some(OperatorCategory::Statement),
            "collection" => Some(OperatorCategory::Collection),
            "object" => Some(OperatorCategory::Object),
            "string" => Some(OperatorCategory::String),
            "regex" => Some(OperatorCategory::Regex),
            "method" => Some(OperatorCategory::Method),
            "nullability" => Some(OperatorCategory::Nullability),
            "membership" => Some(OperatorCategory::Membership),
            "truthiness" => Some(OperatorCategory::Truthiness),
            "collection_boundary" => Some(OperatorCategory::CollectionBoundary),
            "comprehension" => Some(OperatorCategory::Comprehension),
            "dict" => Some(OperatorCategory::Dict),
            "error_handling" => Some(OperatorCategory::ErrorHandling),
            "pattern_matching" => Some(OperatorCategory::PatternMatching),
            "range_boundary" => Some(OperatorCategory::RangeBoundary),
            "return_value" => Some(OperatorCategory::ReturnValue),
            _ => None,
        }
    }

    pub fn operators(self) -> Vec<OperatorName> {
        OperatorName::ALL
            .iter()
            .copied()
            .filter(|op| op.info().category == self)
            .collect()
    }
}

impl std::fmt::Display for OperatorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Language-agnostic metadata for a semantic mutation operator. One row per
/// `OperatorName`; the actual find/apply logic lives in `MutatorImpl` entries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OperatorInfo {
    pub name: &'static str,
    pub category: OperatorCategory,
    pub default_enabled: bool,
    pub description: &'static str,
    pub test_hint: &'static str,
}

impl OperatorName {
    pub const ALL: &'static [OperatorName] = &[
        OperatorName::ComparisonBoundary,
        OperatorName::ComparisonNegation,
        OperatorName::NegateEquality,
        OperatorName::SwapLogical,
        OperatorName::SwapBoolean,
        OperatorName::RemoveNot,
        OperatorName::IntegerZeroOne,
        OperatorName::RangeInclusiveExclusive,
        OperatorName::SwapPredicateMethod,
        OperatorName::NegatePredicateMethod,
        OperatorName::ReturnBoolean,
        OperatorName::IsNoneNegation,
        OperatorName::InNegation,
        OperatorName::TruthinessNegation,
        OperatorName::LenZeroBoundary,
        OperatorName::DictGetDefaultRemoval,
        OperatorName::ComprehensionFilterRemoval,
        OperatorName::NoneReturn,
        OperatorName::EmptyCollectionLiteral,
        OperatorName::IteratorAnyAll,
        OperatorName::MatchBoolPattern,
        OperatorName::OkErrBoolean,
        OperatorName::SomeBoolean,
        OperatorName::OptionSomeNone,
        OperatorName::RemoveTry,
        OperatorName::UnwrapToUnwrapOrDefault,
        OperatorName::MinMaxSwap,
        OperatorName::MatchWildcardToPanic,
        OperatorName::EmptyVecMacro,
        OperatorName::SaturatingCheckedSwap,
        OperatorName::ExpectToUnwrapOrDefault,
        OperatorName::StringBoundaryMethodSwap,
        OperatorName::IncludesNegation,
        OperatorName::SortedReverseFlip,
        OperatorName::DictGetToIndex,
        OperatorName::NullishCoalescingRemoval,
        OperatorName::OptionalChainingRemoval,
        OperatorName::TernaryArmSwap,
        OperatorName::ArrayEmptyLiteral,
        OperatorName::ObjectEmptyLiteral,
        OperatorName::StringEmptyLiteral,
        OperatorName::AwaitRemoval,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            OperatorName::SwapBoolean => "swap_boolean",
            OperatorName::NegateEquality => "negate_equality",
            OperatorName::ComparisonBoundary => "comparison_boundary",
            OperatorName::ComparisonNegation => "comparison_negation",
            OperatorName::SwapLogical => "swap_logical",
            OperatorName::RemoveNot => "remove_not",
            OperatorName::IntegerZeroOne => "integer_zero_one",
            OperatorName::RangeInclusiveExclusive => "range_inclusive_exclusive",
            OperatorName::SwapPredicateMethod => "swap_predicate_method",
            OperatorName::NegatePredicateMethod => "negate_predicate_method",
            OperatorName::ReturnBoolean => "return_boolean",
            OperatorName::IsNoneNegation => "is_none_negation",
            OperatorName::InNegation => "in_negation",
            OperatorName::TruthinessNegation => "truthiness_negation",
            OperatorName::LenZeroBoundary => "len_zero_boundary",
            OperatorName::DictGetDefaultRemoval => "dict_get_default_removal",
            OperatorName::ComprehensionFilterRemoval => "comprehension_filter_removal",
            OperatorName::NoneReturn => "none_return",
            OperatorName::EmptyCollectionLiteral => "empty_collection_literal",
            OperatorName::IteratorAnyAll => "iterator_any_all",
            OperatorName::MatchBoolPattern => "match_bool_pattern",
            OperatorName::OkErrBoolean => "ok_err_boolean",
            OperatorName::SomeBoolean => "some_boolean",
            OperatorName::OptionSomeNone => "option_some_none",
            OperatorName::RemoveTry => "remove_try",
            OperatorName::UnwrapToUnwrapOrDefault => "unwrap_to_unwrap_or_default",
            OperatorName::MinMaxSwap => "min_max_swap",
            OperatorName::MatchWildcardToPanic => "match_wildcard_to_panic",
            OperatorName::EmptyVecMacro => "empty_vec_macro",
            OperatorName::SaturatingCheckedSwap => "saturating_checked_swap",
            OperatorName::ExpectToUnwrapOrDefault => "expect_to_unwrap_or_default",
            OperatorName::StringBoundaryMethodSwap => "string_boundary_method_swap",
            OperatorName::IncludesNegation => "includes_negation",
            OperatorName::SortedReverseFlip => "sorted_reverse_flip",
            OperatorName::DictGetToIndex => "dict_get_to_index",
            OperatorName::NullishCoalescingRemoval => "nullish_coalescing_removal",
            OperatorName::OptionalChainingRemoval => "optional_chaining_removal",
            OperatorName::TernaryArmSwap => "ternary_arm_swap",
            OperatorName::ArrayEmptyLiteral => "array_empty_literal",
            OperatorName::ObjectEmptyLiteral => "object_empty_literal",
            OperatorName::StringEmptyLiteral => "string_empty_literal",
            OperatorName::AwaitRemoval => "await_removal",
        }
    }

    pub fn parse(s: &str) -> Option<OperatorName> {
        OperatorName::ALL.iter().copied().find(|op| op.as_str() == s)
    }

    /// Relative specificity, used to break ties when two operators yield a
    /// byte-identical source edit at the same location (see
    /// `crate::mutate::dedupe_overlapping`). The broad fallback operators
    /// (`swap_boolean` matches every boolean literal; `negate_equality` every
    /// `==`/`!=`) defer to any more specific operator that produces the same
    /// mutant — e.g. `return_boolean` on `return true`, or `len_zero_boundary`
    /// on `len(x) == 0` — so they score lower and are the ones dropped.
    pub fn dedup_priority(self) -> u8 {
        match self {
            OperatorName::SwapBoolean | OperatorName::NegateEquality => 0,
            _ => 1,
        }
    }

    pub fn info(self) -> OperatorInfo {
        match self {
            OperatorName::ComparisonBoundary => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Comparison,
                default_enabled: true,
                description: "Toggle comparison strictness (< <-> <=, > <-> >=).",
                test_hint: "Add boundary tests at the exact threshold value.",
            },
            OperatorName::ComparisonNegation => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Comparison,
                default_enabled: true,
                description: "Negate comparison operators (< -> >=, <= -> >, > -> <=, >= -> <).",
                test_hint: "Add tests covering inputs on both sides of the comparison.",
            },
            OperatorName::NegateEquality => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Equality,
                default_enabled: true,
                description: "Replace == with != or != with ==.",
                test_hint: "Add tests covering equal and non-equal inputs.",
            },
            OperatorName::SwapLogical => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Logical,
                default_enabled: true,
                description: "Replace && with || or || with &&.",
                test_hint: "Add truth-table style tests for both sides of the condition.",
            },
            OperatorName::RemoveNot => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Logical,
                default_enabled: true,
                description: "Remove logical negation (!condition -> condition).",
                test_hint: "Add a test that exercises the negative path of the condition.",
            },
            OperatorName::SwapBoolean => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::BooleanLiteral,
                default_enabled: true,
                description: "Flip boolean literals (true <-> false).",
                test_hint: "Assert both the true and the false branch independently.",
            },
            OperatorName::IntegerZeroOne => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::NumericLiteral,
                default_enabled: false,
                description: "Replace integer 0 with 1 or 1 with 0.",
                test_hint: "Add empty / singleton / boundary count tests.",
            },
            OperatorName::RangeInclusiveExclusive => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::RangeBoundary,
                default_enabled: true,
                description: "Toggle range bound inclusivity (.. <-> ..=).",
                test_hint: "Add a test that exercises the range's final element.",
            },
            OperatorName::SwapPredicateMethod => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: true,
                description: "Swap a predicate method for its opposite (is_some <-> is_none, is_ok <-> is_err).",
                test_hint: "Add tests covering both the present/absent (or ok/err) cases.",
            },
            OperatorName::NegatePredicateMethod => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: true,
                description: "Negate a boolean predicate method call (is_empty() -> !is_empty(), contains(x) -> !contains(x)).",
                test_hint: "Add tests covering both the matching and non-matching cases of the predicate.",
            },
            OperatorName::ReturnBoolean => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ReturnValue,
                default_enabled: true,
                description: "Flip a boolean literal in return position (return true <-> return false).",
                test_hint: "Assert the returned boolean for an input that drives each branch.",
            },
            OperatorName::IsNoneNegation => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Nullability,
                default_enabled: true,
                description: "Toggle a None identity check (x is None <-> x is not None).",
                test_hint: "Add tests covering both the None and the non-None case.",
            },
            OperatorName::InNegation => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Membership,
                default_enabled: true,
                description: "Toggle a membership test (x in y <-> x not in y).",
                test_hint: "Add tests covering both a member and a non-member input.",
            },
            OperatorName::TruthinessNegation => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Truthiness,
                default_enabled: true,
                description: "Negate an if/while condition (if x -> if not x, and back).",
                test_hint: "Add tests that drive the condition both truthy and falsy.",
            },
            OperatorName::LenZeroBoundary => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::CollectionBoundary,
                default_enabled: false,
                description: "Toggle an emptiness check on len (len(x) == 0 <-> != 0, len(x) > 0 -> == 0).",
                test_hint: "Add empty / non-empty collection tests at the boundary.",
            },
            OperatorName::DictGetDefaultRemoval => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Dict,
                default_enabled: false,
                description: "Drop the default from a two-argument dict get (d.get(k, default) -> d.get(k)).",
                test_hint: "Add a test that exercises a missing key so the default matters.",
            },
            OperatorName::ComprehensionFilterRemoval => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Comprehension,
                default_enabled: false,
                description: "Remove a comprehension filter ([x for x in xs if pred(x)] -> [x for x in xs]).",
                test_hint: "Add inputs that the filter is supposed to exclude.",
            },
            OperatorName::NoneReturn => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ReturnValue,
                default_enabled: false,
                description: "Replace a returned value with None (return value -> return None).",
                test_hint: "Assert the concrete returned value, not just truthiness.",
            },
            OperatorName::EmptyCollectionLiteral => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Collection,
                default_enabled: false,
                description: "Empty a collection literal ([a, b] -> [], {a: b} -> {}, {a, b} -> set()).",
                test_hint: "Assert the collection's contents, not just that it is returned.",
            },
            OperatorName::IteratorAnyAll => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: true,
                description: "Swap an iterator predicate quantifier (any(...) <-> all(...)).",
                test_hint: "Add tests with a mix of matching and non-matching elements.",
            },
            OperatorName::MatchBoolPattern => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::PatternMatching,
                default_enabled: true,
                description: "Flip a boolean literal in a match arm pattern (true => <-> false =>).",
                test_hint: "Add tests that drive the scrutinee both true and false.",
            },
            OperatorName::OkErrBoolean => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ErrorHandling,
                default_enabled: true,
                description: "Flip a boolean wrapped in Ok/Err (Ok(true) <-> Ok(false), Err(true) <-> Err(false)).",
                test_hint: "Assert the wrapped boolean, not just that the call returned Ok/Err.",
            },
            OperatorName::SomeBoolean => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Nullability,
                default_enabled: true,
                description: "Flip a boolean wrapped in Some (Some(true) <-> Some(false)).",
                test_hint: "Assert the wrapped boolean, not just that the option was Some.",
            },
            OperatorName::OptionSomeNone => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Nullability,
                default_enabled: false,
                description: "Replace a Some(value) with None (Some(x) -> None).",
                test_hint: "Add a test that distinguishes a present value from None.",
            },
            OperatorName::RemoveTry => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ErrorHandling,
                default_enabled: false,
                description: "Remove the ? operator from a try expression (foo()? -> foo()).",
                test_hint: "Add a test that drives the error path so propagation matters.",
            },
            OperatorName::UnwrapToUnwrapOrDefault => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ErrorHandling,
                default_enabled: false,
                description: "Replace unwrap() with unwrap_or_default() (x.unwrap() -> x.unwrap_or_default()).",
                test_hint: "Add a test on the None/Err case so the panic-vs-default difference shows.",
            },
            OperatorName::MinMaxSwap => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: false,
                description: "Swap a min/max call for its opposite (min <-> max).",
                test_hint: "Add inputs where the smallest and largest values differ.",
            },
            OperatorName::MatchWildcardToPanic => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::PatternMatching,
                default_enabled: false,
                description: "Replace a wildcard match arm's value with a panic (_ => expr -> _ => panic!(..)).",
                test_hint: "Add a test that exercises the fallback/default match arm.",
            },
            OperatorName::EmptyVecMacro => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Collection,
                default_enabled: false,
                description: "Empty a vec! literal (vec![a, b, c] -> vec![]).",
                test_hint: "Assert the vector's contents, not just that it is non-empty.",
            },
            OperatorName::SaturatingCheckedSwap => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Arithmetic,
                default_enabled: false,
                description: "Swap saturating/checked arithmetic (checked_add <-> saturating_add, checked_sub <-> saturating_sub).",
                test_hint: "Add a test at the overflow boundary so saturating and checked behavior differ.",
            },
            OperatorName::ExpectToUnwrapOrDefault => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::ErrorHandling,
                default_enabled: false,
                description: "Replace expect(msg) with unwrap_or_default() (x.expect(\"..\") -> x.unwrap_or_default()).",
                test_hint: "Add a test on the None/Err case so the panic-vs-default difference shows.",
            },
            OperatorName::StringBoundaryMethodSwap => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: true,
                description: "Swap string boundary methods (startsWith <-> endsWith, starts_with <-> ends_with, startswith <-> endswith).",
                test_hint: "Add tests for both matching and non-matching prefixes/suffixes.",
            },
            OperatorName::IncludesNegation => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Membership,
                default_enabled: true,
                description: "Negate an includes/membership predicate.",
                test_hint: "Add tests covering both present and absent values.",
            },
            OperatorName::SortedReverseFlip => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Method,
                default_enabled: false,
                description: "Flip sorted ordering by adding or toggling reverse=...",
                test_hint: "Add tests that assert exact ordering, not just membership.",
            },
            OperatorName::DictGetToIndex => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Dict,
                default_enabled: false,
                description: "Replace d.get(k) with d[k].",
                test_hint: "Add tests that distinguish missing-key behavior from present-key behavior.",
            },
            OperatorName::NullishCoalescingRemoval => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Nullability,
                default_enabled: false,
                description: "Remove fallback from nullish coalescing (a ?? b -> a).",
                test_hint: "Add tests where the left side is null or undefined so the fallback matters.",
            },
            OperatorName::OptionalChainingRemoval => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Nullability,
                default_enabled: false,
                description: "Remove optional chaining (a?.b -> a.b, fn?.() -> fn()).",
                test_hint: "Add tests where the receiver is null or undefined.",
            },
            OperatorName::TernaryArmSwap => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Conditional,
                default_enabled: false,
                description: "Swap ternary result arms (cond ? a : b -> cond ? b : a).",
                test_hint: "Add tests that drive both ternary branches and assert the returned value.",
            },
            OperatorName::ArrayEmptyLiteral => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Collection,
                default_enabled: false,
                description: "Empty an array literal ([a, b] -> []).",
                test_hint: "Assert exact array contents, not just that an array is returned.",
            },
            OperatorName::ObjectEmptyLiteral => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Object,
                default_enabled: false,
                description: "Empty an object literal ({ a: 1 } -> {}).",
                test_hint: "Assert required object properties and values.",
            },
            OperatorName::StringEmptyLiteral => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::String,
                default_enabled: false,
                description: "Replace a non-empty string literal with an empty string.",
                test_hint: "Assert exact strings, especially validation, formatting, and keys.",
            },
            OperatorName::AwaitRemoval => OperatorInfo {
                name: self.as_str(),
                category: OperatorCategory::Statement,
                default_enabled: false,
                description: "Remove await from an awaited expression (await x -> x).",
                test_hint: "Add async tests that assert resolved values and ordering.",
            },
        }
    }
}

impl std::fmt::Display for OperatorName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        f.write_str(&s)
    }
}

/// One language's implementation of a semantic operator: how the idea is found
/// (`query`) and applied (`replacement`) in a single language. Many `MutatorImpl`
/// rows can share an `OperatorName`.
pub struct MutatorImpl {
    /// Stable id of the form `<language>.<operator>`, e.g. `rust.negate_equality`.
    pub id: &'static str,
    pub operator: OperatorName,
    pub language: Language,
    pub query: &'static str,
    pub replacement: fn(&str) -> Option<String>,
    pub description: fn(&str, &str) -> String,
    /// Override the operator-level default for this one language. `None` falls
    /// back to `operator.info().default_enabled`.
    pub default_enabled_override: Option<bool>,
}

impl MutatorImpl {
    pub fn category(&self) -> OperatorCategory {
        self.operator.info().category
    }

    pub fn default_enabled(&self) -> bool {
        self.default_enabled_override
            .unwrap_or(self.operator.info().default_enabled)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutationCandidate {
    pub id: String,
    pub file: PathBuf,
    pub language: Language,
    pub function: String,
    pub operator: OperatorName,
    pub operator_category: OperatorCategory,
    /// Language-qualified implementation id, e.g. `rust.negate_equality`.
    pub implementation: String,
    pub line: usize,
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub original: String,
    pub replacement: String,
    pub description: String,
}

impl FileCoverage {
    pub fn coverage_in_span(&self, start_line: usize, end_line: usize) -> f64 {
        let start = start_line as u32;
        let end = end_line as u32;

        let executable: Vec<_> = self.lines.range(start..=end).collect();

        if executable.is_empty() {
            return 100.0;
        }

        let covered = executable
            .iter()
            .filter(|(_, hits)| **hits > 0)
            .count();

        covered as f64 / executable.len() as f64 * 100.0
    }
}
