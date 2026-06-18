use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/scala/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/scala/branches.scm");

pub struct Scala;

impl Language for Scala {
    fn name(&self) -> &'static str {
        "scala"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scala"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_scala::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
