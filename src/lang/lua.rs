use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/lua/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/lua/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Lua,
    extensions: &["lua"],
    language: || tree_sitter_lua::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
