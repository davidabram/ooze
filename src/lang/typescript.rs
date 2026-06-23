use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/typescript/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/typescript/branches.scm");

pub struct TypeScript;

impl Grammar for TypeScript {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::TypeScript
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
