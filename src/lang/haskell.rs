use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/haskell/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/haskell/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Haskell,
    extensions: &["hs"],
    language: || tree_sitter_haskell::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
