use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/ruby/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/ruby/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Ruby,
    extensions: &["rb"],
    language: || tree_sitter_ruby::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
