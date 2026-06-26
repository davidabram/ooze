use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/php/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/php/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Php,
    extensions: &["php"],
    language: || tree_sitter_php::LANGUAGE_PHP.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
