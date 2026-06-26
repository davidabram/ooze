use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/bash/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/bash/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Bash,
    extensions: &["sh", "bash"],
    language: || tree_sitter_bash::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
