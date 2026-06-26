use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/go/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/go/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Go,
    extensions: &["go"],
    language: || tree_sitter_go::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
