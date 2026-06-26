use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/java/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/java/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Java,
    extensions: &["java"],
    language: || tree_sitter_java::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
