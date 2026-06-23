use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/julia/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/julia/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Julia,
    extensions: &["jl"],
    language: || tree_sitter_julia::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
