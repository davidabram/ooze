use super::LanguageSpec;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/ocaml/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/ocaml/branches.scm");

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Ocaml,
    extensions: &["ml"],
    language: || tree_sitter_ocaml::LANGUAGE_OCAML.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::ScanOnly,
    mutators: &[],
};
