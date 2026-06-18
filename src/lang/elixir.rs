use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/elixir/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/elixir/branches.scm");

pub struct Elixir;

impl Language for Elixir {
    fn name(&self) -> &'static str {
        "elixir"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_elixir::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
