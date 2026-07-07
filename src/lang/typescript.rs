use super::LanguageSpec;
use crate::lang::javascript::{
    empty_array_literal, empty_object_literal, empty_string_literal, negate_js_expression,
    remove_await, remove_nullish_fallback, remove_optional_chaining, swap_ternary_arms,
};
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/typescript/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/typescript/branches.scm");

// TypeScript's mutator implementations (expands to `pub const MUTATORS`). The
// expression grammar matches JavaScript's, so these mirror
// `crate::lang::javascript::MUTATORS` and reuse its helper fns; they are kept as
// a separate slice (with their own query files) so TS-specific tweaks stay
// isolated. The registry aggregates this slice with the other languages'.
mutators! {
    language: TypeScript,
    id_prefix: "typescript",

    SwapBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap boolean literal {original} -> {replacement}")
        },
    },
    NegateEquality {
        replace: |original| match original {
            "==" => Some("!=".to_string()),
            "!=" => Some("==".to_string()),
            "===" => Some("!==".to_string()),
            "!==" => Some("===".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Negate equality {original} -> {replacement}")
        },
    },
    ComparisonBoundary {
        replace: |original| match original {
            "<" => Some("<=".to_string()),
            "<=" => Some("<".to_string()),
            ">" => Some(">=".to_string()),
            ">=" => Some(">".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Toggle comparison boundary {original} -> {replacement}")
        },
    },
    ComparisonNegation {
        replace: |original| match original {
            "<" => Some(">=".to_string()),
            "<=" => Some(">".to_string()),
            ">" => Some("<=".to_string()),
            ">=" => Some("<".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Negate comparison {original} -> {replacement}")
        },
    },
    SwapLogical {
        replace: |original| match original {
            "&&" => Some("||".to_string()),
            "||" => Some("&&".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Swap logical {original} -> {replacement}"),
    },
    IntegerZeroOne {
        replace: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap integer literal {original} -> {replacement}")
        },
    },
    RemoveNot {
        replace: |original| {
            let rest = original.strip_prefix('!')?.trim_start();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        },
        describe: |original, replacement| {
            format!("Remove negation `{original}` -> `{replacement}`")
        },
    },
    ReturnBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip returned boolean {original} -> {replacement}")
        },
    },
    IteratorAnyAll {
        replace: |original| match original {
            "some" => Some("every".to_string()),
            "every" => Some("some".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap iterator quantifier {original}(...) -> {replacement}(...)")
        },
    },
    StringBoundaryMethodSwap {
        replace: |original| match original {
            "startsWith" => Some("endsWith".to_string()),
            "endsWith" => Some("startsWith".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap string boundary method {original}(...) -> {replacement}(...)")
        },
    },
    IncludesNegation {
        replace: negate_js_expression,
        describe: |original, replacement| {
            format!("Negate membership `{original}` -> `{replacement}`")
        },
    },
    NullishCoalescingRemoval {
        replace: remove_nullish_fallback,
        describe: |original, replacement| {
            format!("Remove nullish fallback `{original}` -> `{replacement}`")
        },
    },
    OptionalChainingRemoval {
        replace: remove_optional_chaining,
        describe: |original, replacement| {
            format!("Remove optional chaining `{original}` -> `{replacement}`")
        },
    },
    TernaryArmSwap {
        replace: swap_ternary_arms,
        describe: |original, replacement| {
            format!("Swap ternary arms `{original}` -> `{replacement}`")
        },
    },
    ArrayEmptyLiteral {
        replace: empty_array_literal,
        describe: |original, replacement| {
            format!("Empty array literal `{original}` -> `{replacement}`")
        },
    },
    ObjectEmptyLiteral {
        replace: empty_object_literal,
        describe: |original, replacement| {
            format!("Empty object literal `{original}` -> `{replacement}`")
        },
    },
    StringEmptyLiteral {
        replace: empty_string_literal,
        describe: |original, replacement| {
            format!("Empty string literal `{original}` -> `{replacement}`")
        },
    },
    AwaitRemoval {
        replace: remove_await,
        describe: |original, replacement| {
            format!("Remove await `{original}` -> `{replacement}`")
        },
    },
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::TypeScript,
    extensions: &["ts", "tsx"],
    language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
