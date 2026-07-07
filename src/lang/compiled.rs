//! Query preflight: compile every tree-sitter query a run needs exactly once.
//!
//! Previously function/branch queries were compiled per directory scan and
//! mutator queries were recompiled *inside the per-function discovery loop*. This
//! module compiles each query once, up front, so an invalid query fails fast
//! (before any scanning) and the hot loops receive ready-to-run `Query` handles.
//!
//! A `CompiledRegistry` is built once per run from the static `LanguageSpec`s and
//! threaded into scan and discovery.

use anyhow::{Context, Result};
use tree_sitter::{Language as TsLanguage, Query};

use crate::core::{Language, MutatorImpl};
use crate::lang::LanguageSpec;
use crate::mutate::OperatorFilter;

/// A single mutator's query, compiled against its language grammar, with the
/// `target` capture index resolved once. Mutators whose query has no `target`
/// capture are dropped at compile time (same skip as the old per-function path).
pub struct CompiledMutator {
    pub spec: &'static MutatorImpl,
    pub query: Query,
    pub target_index: u32,
}

/// One language's compiled queries plus the resolved tree-sitter grammar handle,
/// reused for parsing so the loader is not called again per file.
pub struct CompiledLanguage {
    pub spec: &'static LanguageSpec,
    pub ts_language: TsLanguage,
    pub functions: Query,
    pub branches: Query,
    /// Empty for a queries-only registry (scan/crap) or for languages with no
    /// admitted mutators.
    pub mutators: Vec<CompiledMutator>,
}

/// Every language a run will touch, with all queries pre-compiled.
pub struct CompiledRegistry {
    langs: Vec<CompiledLanguage>,
}

impl CompiledRegistry {
    /// Compile function and branch queries for every spec. Mutator queries are
    /// left empty — used by commands that only scan (scan, crap).
    pub fn compile_queries(specs: &[&'static LanguageSpec]) -> Result<Self> {
        let mut langs = Vec::with_capacity(specs.len());
        for &spec in specs {
            langs.push(compile_one(spec)?);
        }
        Ok(Self { langs })
    }

    /// Compile function/branch queries for every spec plus the mutator queries
    /// admitted by `filter`. Fails on the first invalid query.
    pub fn compile(specs: &[&'static LanguageSpec], filter: &OperatorFilter) -> Result<Self> {
        let mut me = Self::compile_queries(specs)?;
        for lang in &mut me.langs {
            for m in lang.spec.mutators.iter().filter(|m| filter.allows_impl(m)) {
                let query = Query::new(&lang.ts_language, m.query)
                    .with_context(|| format!("compiling {} mutation query", m.id))?;
                let Some(target_index) = query.capture_index_for_name("target") else {
                    continue;
                };
                lang.mutators.push(CompiledMutator {
                    spec: m,
                    query,
                    target_index,
                });
            }
        }
        Ok(me)
    }

    pub fn for_language(&self, language: Language) -> Option<&CompiledLanguage> {
        self.langs.iter().find(|c| c.spec.id == language)
    }

    /// Look up the compiled language registered for a file extension.
    pub fn for_extension(&self, ext: &str) -> Option<&CompiledLanguage> {
        self.langs.iter().find(|c| c.spec.extensions.contains(&ext))
    }

    // Threaded into file-grouped discovery in a later step; for now only the
    // compile preflight tests iterate the whole set.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn languages(&self) -> &[CompiledLanguage] {
        &self.langs
    }
}

fn compile_one(spec: &'static LanguageSpec) -> Result<CompiledLanguage> {
    let ts_language = (spec.language)();
    let functions = Query::new(&ts_language, spec.functions_query)
        .with_context(|| format!("compiling {} function query", spec.name()))?;
    let branches = Query::new(&ts_language, spec.branches_query)
        .with_context(|| format!("compiling {} branch query", spec.name()))?;
    Ok(CompiledLanguage {
        spec,
        ts_language,
        functions,
        branches,
        mutators: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_all_languages_with_all_mutators_succeeds() {
        // Strengthens the old `all_mutator_queries_compile`: every function,
        // branch, and mutator query across every language compiles in one pass.
        let reg = CompiledRegistry::compile(
            crate::lang::supported_languages(),
            &OperatorFilter::allow_all(),
        )
        .expect("all queries should compile");

        let total_mutators: usize = reg.languages().iter().map(|l| l.mutators.len()).sum();
        assert!(total_mutators > 0, "expected at least one compiled mutator");

        // Every compiled mutator resolved its `target` capture.
        for lang in reg.languages() {
            for m in &lang.mutators {
                let names = m.query.capture_names();
                assert_eq!(
                    names[m.target_index as usize], "target",
                    "{}: target_index must point at the `target` capture",
                    m.spec.id
                );
            }
        }
    }

    #[test]
    fn compile_queries_omits_mutators() {
        let reg = CompiledRegistry::compile_queries(crate::lang::supported_languages()).unwrap();
        assert!(reg.languages().iter().all(|l| l.mutators.is_empty()));
    }

    #[test]
    fn invalid_query_fails_fast_with_named_context() {
        // A spec with a syntactically broken function query must error at compile
        // time, naming the language so the failure is actionable.
        const BROKEN: LanguageSpec = LanguageSpec {
            id: Language::Rust,
            extensions: &["rs"],
            language: || tree_sitter_rust::LANGUAGE.into(),
            functions_query: "(this is not a valid query",
            branches_query: "",
            support: crate::core::SupportLevel::MutateExperimental,
            mutators: &[],
        };
        let err = CompiledRegistry::compile_queries(&[&BROKEN])
            .err()
            .expect("broken query must fail to compile");
        assert!(
            format!("{err:#}").contains("function query"),
            "error should mention the function query: {err:#}"
        );
    }

    #[test]
    fn for_extension_and_for_language_resolve() {
        let reg = CompiledRegistry::compile_queries(crate::lang::supported_languages()).unwrap();
        assert_eq!(
            reg.for_extension("rs").map(|c| c.spec.id),
            Some(Language::Rust)
        );
        assert!(reg.for_language(Language::Rust).is_some());
        assert!(reg.for_extension("nope-no-such-ext").is_none());
    }
}
