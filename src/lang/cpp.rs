use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/cpp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/cpp/branches.scm");

pub struct Cpp;

impl Language for Cpp {
    fn name(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cc", "cxx", "hpp", "hh", "hxx"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
