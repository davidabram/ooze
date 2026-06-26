use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/scala/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/scala/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Scala,
    extensions: &["scala"],
    language: || tree_sitter_scala::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
