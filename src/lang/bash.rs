use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/bash/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/bash/branches.scm");

pub struct Bash;

impl Grammar for Bash {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Bash
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_bash::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
