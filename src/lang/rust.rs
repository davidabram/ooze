use super::Language;
use crate::core::{MutationOperator, OperatorName};

const FUNCTIONS_QUERY: &str = include_str!("../../queries/rust/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/rust/branches.scm");

const SWAP_BOOLEAN: MutationOperator = MutationOperator {
    name: OperatorName::SwapBoolean,
    query: include_str!("../../queries/rust/swap-boolean.scm"),
    replacement: |original| match original {
        "true" => Some("false".to_string()),
        "false" => Some("true".to_string()),
        _ => None,
    },
    description: |original, replacement| {
        format!("Swap boolean literal {original} -> {replacement}")
    },
};

const NEGATE_EQUALITY: MutationOperator = MutationOperator {
    name: OperatorName::NegateEquality,
    query: include_str!("../../queries/rust/negate-equality.scm"),
    replacement: |original| match original {
        "==" => Some("!=".to_string()),
        "!=" => Some("==".to_string()),
        _ => None,
    },
    description: |original, replacement| {
        format!("Negate equality {original} -> {replacement}")
    },
};

const COMPARISON_BOUNDARY: MutationOperator = MutationOperator {
    name: OperatorName::ComparisonBoundary,
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
};

const COMPARISON_NEGATION: MutationOperator = MutationOperator {
    name: OperatorName::ComparisonNegation,
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
};

const SWAP_LOGICAL: MutationOperator = MutationOperator {
    name: OperatorName::SwapLogical,
    query: include_str!("../../queries/rust/swap-logical.scm"),
    replacement: |original| match original {
        "&&" => Some("||".to_string()),
        "||" => Some("&&".to_string()),
        _ => None,
    },
    description: |original, replacement| {
        format!("Swap logical {original} -> {replacement}")
    },
};

const REMOVE_NOT: MutationOperator = MutationOperator {
    name: OperatorName::RemoveNot,
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
};

const INTEGER_ZERO_ONE: MutationOperator = MutationOperator {
    name: OperatorName::IntegerZeroOne,
    query: include_str!("../../queries/rust/integer-zero-one.scm"),
    replacement: |original| match original {
        "0" => Some("1".to_string()),
        "1" => Some("0".to_string()),
        _ => None,
    },
    description: |original, replacement| {
        format!("Replace integer {original} -> {replacement}")
    },
};

const MUTATION_OPERATORS: &[MutationOperator] = &[
    SWAP_BOOLEAN,
    NEGATE_EQUALITY,
    COMPARISON_BOUNDARY,
    COMPARISON_NEGATION,
    SWAP_LOGICAL,
    REMOVE_NOT,
    INTEGER_ZERO_ONE,
];

pub struct Rust;

impl Language for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }

    fn mutation_operators(&self) -> &'static [MutationOperator] {
        MUTATION_OPERATORS
    }
}
