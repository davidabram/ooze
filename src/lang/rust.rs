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

const MUTATION_OPERATORS: &[MutationOperator] = &[SWAP_BOOLEAN];

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
