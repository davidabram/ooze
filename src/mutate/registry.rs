//! Aggregates the per-language mutator implementations into one lookup point.
//!
//! A semantic operator (`OperatorName`) can have one implementation per language.
//! The implementations themselves live next to each grammar (e.g.
//! `crate::lang::rust::MUTATORS`); this module just chains those slices so
//! discovery can ask for "every implementation registered for language X"
//! without knowing which file they came from.

use crate::core::{Language, MutatorImpl};

/// Every language's mutator slice. Add a language by appending its slice here.
const LANGUAGE_MUTATORS: &[&[MutatorImpl]] = &[crate::lang::rust::MUTATORS];

/// Every registered mutator implementation across all languages.
pub fn all() -> impl Iterator<Item = &'static MutatorImpl> {
    LANGUAGE_MUTATORS.iter().copied().flatten()
}

/// Implementations registered for a given language.
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
        assert_eq!(count, 11, "expected all eleven rust operators");
    }
}
