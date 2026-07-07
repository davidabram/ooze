use super::LanguageSpec;
use crate::core::Language;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/go/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/go/branches.scm");

// Go's initial mutation operator set: literal/operator swaps only. These queries
// match syntax nodes, so `true` in a comment or `==` inside a string literal can
// never produce a candidate. Deliberately excluded for now: statement deletion,
// return value replacement, nil insertion — anything likely to produce
// non-compiling mutants.
mutators! {
    language: Go,
    id_prefix: "go",

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
        describe: |original, replacement| format!("Replace integer {original} -> {replacement}"),
    },
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: Language::Go,
    extensions: &["go"],
    language: || tree_sitter_go::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
