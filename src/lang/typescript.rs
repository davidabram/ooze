use super::GrammarDef;
use crate::core::{Language, MutatorImpl, OperatorName};

const FUNCTIONS_QUERY: &str = include_str!("../../queries/typescript/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/typescript/branches.scm");

/// TypeScript's mutator implementations. The expression grammar matches
/// JavaScript's, so these mirror `crate::lang::javascript::MUTATORS`; they are
/// kept as a separate slice (with their own query files) so TS-specific tweaks
/// stay isolated. The registry (`crate::mutate::registry`) aggregates this slice
/// with the other languages' slices for discovery.
pub const MUTATORS: &[MutatorImpl] = &[
    MutatorImpl {
        id: "typescript.swap_boolean",
        operator: OperatorName::SwapBoolean,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/swap-boolean.scm"),
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
        id: "typescript.negate_equality",
        operator: OperatorName::NegateEquality,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/negate-equality.scm"),
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
        id: "typescript.comparison_boundary",
        operator: OperatorName::ComparisonBoundary,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/comparison-boundary.scm"),
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
        id: "typescript.comparison_negation",
        operator: OperatorName::ComparisonNegation,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/comparison-negation.scm"),
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
        id: "typescript.swap_logical",
        operator: OperatorName::SwapLogical,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/swap-logical.scm"),
        replacement: |original| match original {
            "&&" => Some("||".to_string()),
            "||" => Some("&&".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap logical {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "typescript.remove_not",
        operator: OperatorName::RemoveNot,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/remove-not.scm"),
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
        id: "typescript.return_boolean",
        operator: OperatorName::ReturnBoolean,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/return-boolean.scm"),
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
    MutatorImpl {
        id: "typescript.iterator_any_all",
        operator: OperatorName::IteratorAnyAll,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/iterator-any-all.scm"),
        replacement: |original| match original {
            "some" => Some("every".to_string()),
            "every" => Some("some".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap iterator quantifier {original}(...) -> {replacement}(...)")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "typescript.string_boundary_method_swap",
        operator: OperatorName::StringBoundaryMethodSwap,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/string-boundary-method-swap.scm"),
        replacement: |original| match original {
            "startsWith" => Some("endsWith".to_string()),
            "endsWith" => Some("startsWith".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap string boundary method {original}(...) -> {replacement}(...)")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "typescript.includes_negation",
        operator: OperatorName::IncludesNegation,
        language: Language::TypeScript,
        query: include_str!("../../queries/typescript/includes-negation.scm"),
        replacement: crate::lang::javascript::negate_js_expression,
        description: |original, replacement| {
            format!("Negate membership `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
];

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::TypeScript,
    extensions: &["ts", "tsx"],
    language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
