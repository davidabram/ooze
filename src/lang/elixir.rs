use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/elixir/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/elixir/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Elixir,
    extensions: &["ex", "exs"],
    language: || tree_sitter_elixir::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
