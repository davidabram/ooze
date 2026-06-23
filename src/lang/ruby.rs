use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/ruby/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/ruby/branches.scm");

pub struct Ruby;

impl Grammar for Ruby {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Ruby
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
