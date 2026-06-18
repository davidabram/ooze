use super::Language;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/hcl/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/hcl/branches.scm");

pub struct Hcl;

impl Language for Hcl {
    fn name(&self) -> &'static str {
        "hcl"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hcl", "tf", "tfvars"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_hcl::LANGUAGE.into()
    }

    fn functions_query(&self) -> &'static str {
        FUNCTIONS_QUERY
    }

    fn branches_query(&self) -> &'static str {
        BRANCHES_QUERY
    }
}
