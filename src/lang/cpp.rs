use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/cpp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/cpp/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Cpp,
    extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
    language: || tree_sitter_cpp::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
