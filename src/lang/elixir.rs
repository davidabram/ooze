use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/elixir/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/elixir/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Elixir,
    extensions: &["ex", "exs"],
    language: || tree_sitter_elixir::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
