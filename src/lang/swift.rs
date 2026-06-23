use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/swift/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/swift/branches.scm");

pub struct Swift;

impl Grammar for Swift {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Swift
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
