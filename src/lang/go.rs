use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/go/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/go/branches.scm");

pub struct Go;

impl Language for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
