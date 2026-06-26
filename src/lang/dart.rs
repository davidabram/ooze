use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/dart/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/dart/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Dart,
    extensions: &["dart"],
    language: || tree_sitter_dart::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
