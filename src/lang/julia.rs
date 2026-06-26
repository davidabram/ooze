use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/julia/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/julia/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Julia,
    extensions: &["jl"],
    language: || tree_sitter_julia::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
