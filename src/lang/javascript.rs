use super::Grammar;

const FUNCTIONS_QUERY: &str =
    include_str!("../../queries/javascript/functions.scm");
const BRANCHES_QUERY: &str =
    include_str!("../../queries/javascript/branches.scm");

pub struct JavaScript;

impl Grammar for JavaScript {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::JavaScript
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
