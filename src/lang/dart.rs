use super::Grammar;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/dart/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/dart/branches.scm");

pub struct Dart;

impl Grammar for Dart {
    fn id(&self) -> crate::core::Language {
        crate::core::Language::Dart
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_dart::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
