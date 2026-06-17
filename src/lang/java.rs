use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/java/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/java/branches.scm");

pub struct Java;

impl Language for Java {
    fn name(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
