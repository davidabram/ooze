use super::GrammarDef;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/typescript/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/typescript/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::TypeScript,
    extensions: &["ts", "tsx"],
    language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
