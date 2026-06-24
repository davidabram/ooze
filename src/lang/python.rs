use super::GrammarDef;
use crate::core::{Language, MutatorImpl, OperatorName};

const FUNCTIONS_QUERY: &str = include_str!("../../queries/python/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/python/branches.scm");

/// Python's mutator implementations. The registry (`crate::mutate::registry`)
/// aggregates this slice with the other languages' slices for discovery.
///
/// This is the generic-syntax MVP set: boolean literals, equality, comparison,
/// and logical operators. Python-specific operators (`is None` negation, `in`
/// negation, truthiness, etc.) can be layered on later as additional entries.
pub const MUTATORS: &[MutatorImpl] = &[
    MutatorImpl {
        id: "python.swap_boolean",
        operator: OperatorName::SwapBoolean,
        language: Language::Python,
        query: include_str!("../../queries/python/swap-boolean.scm"),
        replacement: |original| match original {
            "True" => Some("False".to_string()),
            "False" => Some("True".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap boolean literal {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.negate_equality",
        operator: OperatorName::NegateEquality,
        language: Language::Python,
        query: include_str!("../../queries/python/negate-equality.scm"),
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
        id: "python.comparison_boundary",
        operator: OperatorName::ComparisonBoundary,
        language: Language::Python,
        query: include_str!("../../queries/python/comparison-boundary.scm"),
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
        id: "python.comparison_negation",
        operator: OperatorName::ComparisonNegation,
        language: Language::Python,
        query: include_str!("../../queries/python/comparison-negation.scm"),
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
        id: "python.swap_logical",
        operator: OperatorName::SwapLogical,
        language: Language::Python,
        query: include_str!("../../queries/python/swap-logical.scm"),
        replacement: |original| match original {
            "and" => Some("or".to_string()),
            "or" => Some("and".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap logical {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.integer_zero_one",
        operator: OperatorName::IntegerZeroOne,
        language: Language::Python,
        query: include_str!("../../queries/python/integer-zero-one.scm"),
        replacement: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap integer literal {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
];

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Python,
    extensions: &["py"],
    language: || tree_sitter_python::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
