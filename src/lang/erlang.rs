use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/erlang/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/erlang/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Erlang,
    extensions: &["erl"],
    language: || tree_sitter_erlang::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
