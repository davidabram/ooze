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
    MutatorImpl {
        id: "javascript.return_boolean",
        operator: OperatorName::ReturnBoolean,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/return-boolean.scm"),
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
        id: "javascript.iterator_any_all",
        operator: OperatorName::IteratorAnyAll,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/iterator-any-all.scm"),
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
        id: "javascript.string_boundary_method_swap",
        operator: OperatorName::StringBoundaryMethodSwap,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/string-boundary-method-swap.scm"),
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
        id: "javascript.includes_negation",
        operator: OperatorName::IncludesNegation,
        language: Language::JavaScript,
        query: include_str!("../../queries/javascript/includes-negation.scm"),
        replacement: negate_js_expression,
        description: |original, replacement| {
            format!("Negate membership `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
];

/// `includes_negation`: flip an `includes` membership predicate by wrapping it in
/// `!(...)`. An existing leading `!` is unwrapped so the mutation toggles
/// cleanly; the parentheses keep operator precedence intact.
pub(crate) fn negate_js_expression(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('!') {
        let inner = rest.trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        Some(format!("!({trimmed})"))
    }
}

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::JavaScript,
    extensions: &["js", "jsx", "mjs", "cjs"],
    language: || tree_sitter_javascript::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
