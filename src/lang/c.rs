use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::C,
    extensions: &["c", "h"],
    language: || tree_sitter_c::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
