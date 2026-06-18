use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/ocaml/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/ocaml/branches.scm");

pub struct Ocaml;

impl Language for Ocaml {
    fn name(&self) -> &'static str {
        "ocaml"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ml"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_ocaml::LANGUAGE_OCAML.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
