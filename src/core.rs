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
        }
    }

    pub fn parse(s: &str) -> Option<OperatorName> {
        OperatorName::ALL.iter().copied().find(|op| op.as_str() == s)
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
