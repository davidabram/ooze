use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/julia/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/julia/branches.scm");

pub struct Julia;

impl Language for Julia {
    fn name(&self) -> &'static str {
        "julia"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["jl"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_julia::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
