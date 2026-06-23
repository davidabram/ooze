use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c_sharp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c_sharp/branches.scm");

#[allow(non_camel_case_types)]
pub struct CSharp;

impl Grammar for CSharp {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::CSharp
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
