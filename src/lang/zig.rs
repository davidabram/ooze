use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/zig/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/zig/branches.scm");

pub struct Zig;

impl Grammar for Zig {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Zig
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_zig::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
