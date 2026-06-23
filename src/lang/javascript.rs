use super::GrammarDef;

const FUNCTIONS_QUERY: &str =
    include_str!("../../queries/javascript/functions.scm");
const BRANCHES_QUERY: &str =
    include_str!("../../queries/javascript/branches.scm");

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::JavaScript,
    extensions: &["js", "jsx", "mjs", "cjs"],
    language: || tree_sitter_javascript::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
