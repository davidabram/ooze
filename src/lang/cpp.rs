use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/cpp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/cpp/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Cpp,
    extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
    language: || tree_sitter_cpp::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
