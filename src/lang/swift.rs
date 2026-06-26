use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/swift/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/swift/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Swift,
    extensions: &["swift"],
    language: || tree_sitter_swift::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
