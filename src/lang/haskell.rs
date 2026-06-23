use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/haskell/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/haskell/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Haskell,
    extensions: &["hs"],
    language: || tree_sitter_haskell::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
