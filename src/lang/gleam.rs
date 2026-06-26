use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/gleam/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/gleam/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Gleam,
    extensions: &["gleam"],
    language: || tree_sitter_gleam::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
