use super::GrammarDef;
use crate::core::{Language, MutatorImpl, OperatorName};

const FUNCTIONS_QUERY: &str = include_str!("../../queries/rust/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/rust/branches.scm");

/// Rust's mutator implementations. The registry (`crate::mutate::registry`)
/// aggregates this slice with the other languages' slices for discovery.
pub const MUTATORS: &[MutatorImpl] = &[
    MutatorImpl {
        id: "rust.swap_boolean",
        operator: OperatorName::SwapBoolean,
        language: Language::Rust,
        query: include_str!("../../queries/rust/swap-boolean.scm"),
        replacement: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap boolean literal {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.negate_equality",
        operator: OperatorName::NegateEquality,
        language: Language::Rust,
        query: include_str!("../../queries/rust/negate-equality.scm"),
        replacement: |original| match original {
            "==" => Some("!=".to_string()),
            "!=" => Some("==".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Negate equality {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.comparison_boundary",
        operator: OperatorName::ComparisonBoundary,
        language: Language::Rust,
        query: include_str!("../../queries/rust/comparison-boundary.scm"),
        replacement: |original| match original {
            "<" => Some("<=".to_string()),
            "<=" => Some("<".to_string()),
            ">" => Some(">=".to_string()),
            ">=" => Some(">".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Toggle comparison boundary {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.comparison_negation",
        operator: OperatorName::ComparisonNegation,
        language: Language::Rust,
        query: include_str!("../../queries/rust/comparison-negation.scm"),
        replacement: |original| match original {
            "<" => Some(">=".to_string()),
            "<=" => Some(">".to_string()),
            ">" => Some("<=".to_string()),
            ">=" => Some("<".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Negate comparison {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.swap_logical",
        operator: OperatorName::SwapLogical,
        language: Language::Rust,
        query: include_str!("../../queries/rust/swap-logical.scm"),
        replacement: |original| match original {
            "&&" => Some("||".to_string()),
            "||" => Some("&&".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap logical {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.remove_not",
        operator: OperatorName::RemoveNot,
        language: Language::Rust,
        query: include_str!("../../queries/rust/remove-not.scm"),
        replacement: |original| {
            let rest = original.strip_prefix('!')?.trim_start();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        },
        description: |original, replacement| {
            format!("Remove negation `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.integer_zero_one",
        operator: OperatorName::IntegerZeroOne,
        language: Language::Rust,
        query: include_str!("../../queries/rust/integer-zero-one.scm"),
        replacement: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Replace integer {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.range_inclusive_exclusive",
        operator: OperatorName::RangeInclusiveExclusive,
        language: Language::Rust,
        query: include_str!("../../queries/rust/range-inclusive-exclusive.scm"),
        replacement: |original| match original {
            ".." => Some("..=".to_string()),
            "..=" => Some("..".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Toggle range bound {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.swap_predicate_method",
        operator: OperatorName::SwapPredicateMethod,
        language: Language::Rust,
        query: include_str!("../../queries/rust/swap-predicate-method.scm"),
        replacement: |original| match original {
            "is_some" => Some("is_none".to_string()),
            "is_none" => Some("is_some".to_string()),
            "is_ok" => Some("is_err".to_string()),
            "is_err" => Some("is_ok".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap predicate method {original}() -> {replacement}()")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.negate_predicate_method",
        operator: OperatorName::NegatePredicateMethod,
        language: Language::Rust,
        query: include_str!("../../queries/rust/negate-predicate-method.scm"),
        // The query only matches bool-returning predicate calls, so wrapping the
        // whole call expression in `!` is always type-correct.
        replacement: |original| Some(format!("!{original}")),
        description: |original, replacement| {
            format!("Negate predicate method `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "rust.return_boolean",
        operator: OperatorName::ReturnBoolean,
        language: Language::Rust,
        query: include_str!("../../queries/rust/return-boolean.scm"),
        replacement: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Flip returned boolean {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
];

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Rust,
    extensions: &["rs"],
    language: || tree_sitter_rust::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
