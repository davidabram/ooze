use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/lua/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/lua/branches.scm");

pub struct Lua;

impl Grammar for Lua {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Lua
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_lua::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
