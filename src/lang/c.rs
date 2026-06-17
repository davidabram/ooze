use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c/branches.scm");

pub struct C;

impl Language for C {
    fn name(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
