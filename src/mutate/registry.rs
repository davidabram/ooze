//! One lookup point over the per-language mutator implementations.
//!
//! A semantic operator (`OperatorName`) can have one implementation per language.
//! The implementations live on each language's `LanguageSpec` (its `mutators`
//! field), so there is no separate registry list to keep in sync: this module
//! just walks `crate::lang::LANGUAGES` — the single source of truth — and answers
//! "every implementation registered for language X".

use crate::core::{Language, MutatorImpl};

/// Every registered mutator implementation across all languages, sourced from the
/// per-language spec definitions in `crate::lang::LANGUAGES`.
pub fn all() -> impl Iterator<Item = &'static MutatorImpl> {
    crate::lang::LANGUAGES
        .iter()
        .flat_map(|spec| spec.mutators.iter())
}

/// Implementations registered for a given language, used by `doctor --operators`
/// and tests.
pub fn implementations_for_language(
    language: Language,
) -> impl Iterator<Item = &'static MutatorImpl> {
    all().filter(move |m| m.language == language)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn impl_ids_are_language_qualified_and_unique() {
        let mut seen = HashSet::new();
        for m in all() {
            let expected = format!("{}.{}", m.language.as_str(), m.operator.as_str());
            assert_eq!(m.id, expected, "impl id must be <language>.<operator>");
            assert!(seen.insert(m.id), "duplicate impl id {}", m.id);
        }
    }

    #[test]
    fn rust_registers_every_current_operator() {
        let count = implementations_for_language(Language::Rust).count();
        assert_eq!(count, 23, "expected all twenty-three rust operators");
    }

    #[test]
    fn csharp_registers_every_current_operator() {
        let count = implementations_for_language(Language::CSharp).count();
        assert_eq!(count, 23, "expected all twenty-three c_sharp operators");
    }

    #[test]
    fn support_level_agrees_with_mutators() {
        for spec in crate::lang::LANGUAGES {
            assert_eq!(
                spec.support.mutates(),
                !spec.mutators.is_empty(),
                "{}: support level and presence of mutators disagree",
                spec.id
            );
        }
    }

    #[test]
    fn all_mutator_queries_compile() {
        for m in all() {
            let spec = crate::lang::spec_for_language(m.language)
                .unwrap_or_else(|| panic!("{}: no spec registered for {}", m.id, m.language));
            let ts_lang = (spec.language)();
            tree_sitter::Query::new(&ts_lang, m.query)
                .unwrap_or_else(|e| panic!("{} query failed to compile: {e}", m.id));
        }
    }
}
