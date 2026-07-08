use super::LanguageSpec;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c_sharp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c_sharp/branches.scm");

// C#'s initial mutation operator set: literal/operator swaps only, mirroring
// Go's baseline plus comparison negation. The queries match syntax nodes
// (`boolean_literal`, `integer_literal`, `binary_expression` operators), so
// `true` in a comment or `==` inside a string literal can never produce a
// candidate. Deliberately excluded for now: null insertion, default(T)
// replacement, LINQ/async rewrites, pattern matching — anything likely to
// produce non-compiling or noisy mutants.
mutators! {
    language: CSharp,
    id_prefix: "c_sharp",

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
        describe: |original, replacement| format!("Replace integer {original} -> {replacement}"),
    },
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::CSharp,
    extensions: &["cs"],
    language: || tree_sitter_c_sharp::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
