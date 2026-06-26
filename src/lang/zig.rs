use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/zig/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/zig/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Zig,
    extensions: &["zig"],
    language: || tree_sitter_zig::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
