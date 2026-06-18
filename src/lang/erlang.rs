use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/erlang/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/erlang/branches.scm");

pub struct Erlang;

impl Language for Erlang {
    fn name(&self) -> &'static str {
        "erlang"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["erl"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_erlang::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
