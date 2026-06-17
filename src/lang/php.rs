use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/php/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/php/branches.scm");

pub struct Php;

impl Language for Php {
    fn name(&self) -> &'static str {
        "php"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_php::LANGUAGE_PHP.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
