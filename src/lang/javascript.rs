use super::GrammarDef;
use crate::core::{Language, MutatorImpl, OperatorName};

const FUNCTIONS_QUERY: &str =
    include_str!("../../queries/javascript/functions.scm");
const BRANCHES_QUERY: &str =
    include_str!("../../queries/javascript/branches.scm");

/// JavaScript's mutator implementations. The registry
/// (`crate::mutate::registry`) aggregates this slice with the other languages'
/// slices for discovery.
pub const MUTATORS: &[MutatorImpl] = &[
    MutatorImpl {
        id: "javascript.swap_boolean",
        operator: OperatorName::SwapBoolean,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/swap-boolean.scm"),
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
        id: "javascript.negate_equality",
        operator: OperatorName::NegateEquality,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/negate-equality.scm"),
        replacement: |original| match original {
            "==" => Some("!=".to_string()),
            "!=" => Some("==".to_string()),
            "===" => Some("!==".to_string()),
            "!==" => Some("===".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Negate equality {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "javascript.comparison_boundary",
        operator: OperatorName::ComparisonBoundary,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/comparison-boundary.scm"),
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
        id: "javascript.comparison_negation",
        operator: OperatorName::ComparisonNegation,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/comparison-negation.scm"),
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
        id: "javascript.swap_logical",
        operator: OperatorName::SwapLogical,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/swap-logical.scm"),
        replacement: |original| match original {
            "&&" => Some("||".to_string()),
            "||" => Some("&&".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap logical {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "javascript.remove_not",
        operator: OperatorName::RemoveNot,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/remove-not.scm"),
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
];

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::JavaScript,
    extensions: &["js", "jsx", "mjs", "cjs"],
    language: || tree_sitter_javascript::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
