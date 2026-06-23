use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/python/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/python/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Python,
    extensions: &["py"],
    language: || tree_sitter_python::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
