use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c_sharp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c_sharp/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::CSharp,
    extensions: &["cs"],
    language: || tree_sitter_c_sharp::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
