use super::LanguageSpec;
use crate::lang::javascript::empty_string_literal;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c_sharp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c_sharp/branches.scm");

// C#'s mutation operator set: literal/operator swaps plus arithmetic,
// compound assignment, and unary mutations. The queries match syntax nodes
// (`boolean_literal`, `binary_expression` operators, `prefix_unary_expression`,
// `assignment_expression`), so `true` in a comment or `==` inside a string
// literal can never produce a candidate — except `string_empty_literal`, which
// intentionally targets regular `string_literal` nodes and is disabled by
// default. Deliberately excluded for now: plain `=` and `%=` assignment, null
// insertion, default(T) replacement, LINQ/async rewrites, pattern matching —
// anything likely to produce non-compiling or noisy mutants.
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
    SwapArithmetic {
        replace: |original| match original {
            "+" => Some("-".to_string()),
            "-" => Some("+".to_string()),
            "*" => Some("/".to_string()),
            "/" | "%" => Some("*".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap arithmetic operator {original} -> {replacement}")
        },
    },
    SwapAssignment {
        replace: |original| match original {
            "+=" => Some("-=".to_string()),
            "-=" => Some("+=".to_string()),
            "*=" => Some("/=".to_string()),
            "/=" => Some("*=".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap assignment operator {original} -> {replacement}")
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
    RemoveUnaryMinus {
        replace: |original| {
            let rest = original.strip_prefix('-')?.trim_start();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        },
        describe: |original, replacement| {
            format!("Remove unary minus `{original}` -> `{replacement}`")
        },
    },
    PlusToMinus {
        replace: |original| match original {
            "+" => Some("-".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Replace unary plus {original} -> {replacement}")
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
    StringEmptyLiteral {
        replace: empty_string_literal,
        describe: |original, replacement| {
            format!("Empty string literal `{original}` -> `{replacement}`")
        },
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
