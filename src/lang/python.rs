use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/python/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/python/branches.scm");

pub struct Python;

impl Grammar for Python {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Python
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
