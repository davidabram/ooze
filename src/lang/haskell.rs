use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/haskell/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/haskell/branches.scm");

pub struct Haskell;

impl Grammar for Haskell {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Haskell
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_haskell::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
