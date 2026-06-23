use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/gleam/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/gleam/branches.scm");

pub struct Gleam;

impl Grammar for Gleam {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Gleam
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gleam"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_gleam::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
