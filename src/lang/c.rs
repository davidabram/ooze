use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c/branches.scm");

pub struct C;

impl Grammar for C {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::C
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
