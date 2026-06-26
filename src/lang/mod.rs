use std::path::Path;

use anyhow::Context;
use streaming_iterator::StreamingIterator;

use crate::core::{FunctionSpan, Language, MutatorImpl, SupportLevel};

mod mutator_macro;
pub(crate) use mutator_macro::mutators;

mod compiled;
pub use compiled::{CompiledLanguage, CompiledMutator, CompiledRegistry};

pub mod bash;
pub mod c;
pub mod c_sharp;
pub mod cpp;
pub mod dart;
pub mod elixir;
pub mod erlang;
pub mod gleam;
pub mod go;
pub mod haskell;
pub mod java;
pub mod javascript;
pub mod julia;
pub mod lua;
pub mod ocaml;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod typescript;
pub mod zig;

#[cfg(test)]
mod tests;

/// The full comptime description of a language: how to parse it, which mutation
/// operators it ships, and how far its support is trusted. This is the single
/// source of truth — `LANGUAGES` is the only place a language is registered, and
/// `crate::mutate::registry` derives the mutator lookup from `mutators` here
/// rather than maintaining a parallel list. A compile-time constant per language
/// (e.g. `crate::lang::rust::SPEC`).
pub struct LanguageSpec {
    /// The typed language id. `name()` derives from this, so it is the single
    /// source of truth for the canonical language string.
    pub id: Language,
    pub extensions: &'static [&'static str],
    /// Loads the tree-sitter language. A function pointer because the grammar
    /// handle is produced by a runtime call, not a const.
    pub language: fn() -> tree_sitter::Language,
    pub functions_query: &'static str,
    pub branches_query: &'static str,
    /// How far support for this language goes. Must agree with `mutators`: a
    /// `ScanOnly` language has no mutators; a `Mutate*` language has at least one.
    pub support: SupportLevel,
    /// Mutation operators registered for this language, or `&[]` for scan-only
    /// languages.
    pub mutators: &'static [MutatorImpl],
}

impl LanguageSpec {
    pub fn name(&self) -> &'static str {
        self.id.as_str()
    }
}

pub const LANGUAGES: &[&LanguageSpec] = &[
    &bash::SPEC,
    &javascript::SPEC,
    &typescript::SPEC,
    &python::SPEC,
    &java::SPEC,
    &c_sharp::SPEC,
    &cpp::SPEC,
    &c::SPEC,
    &dart::SPEC,
    &elixir::SPEC,
    &erlang::SPEC,
    &gleam::SPEC,
    &go::SPEC,
    &haskell::SPEC,
    &julia::SPEC,
    &lua::SPEC,
    &ocaml::SPEC,
    &rust::SPEC,
    &ruby::SPEC,
    &php::SPEC,
    &scala::SPEC,
    &swift::SPEC,
    &zig::SPEC,
];

pub fn supported_languages() -> &'static [&'static LanguageSpec] {
    LANGUAGES
}

/// The grammar registered for a language, if any. Used by mutator tests to pair
/// a `MutatorImpl` with the tree-sitter language its query must compile against,
/// and anywhere else that needs to go from a typed `Language` back to its parser.
#[cfg_attr(not(test), allow(dead_code))]
pub fn spec_for_language(language: Language) -> Option<&'static LanguageSpec> {
    LANGUAGES.iter().copied().find(|g| g.id == language)
}

pub fn scan_directory(path: &Path) -> anyhow::Result<Vec<FunctionSpan>> {
    scan_directory_with_excludes(path, &[])
}

pub fn scan_directory_with_excludes(
    path: &Path,
    excludes: &[String],
) -> anyhow::Result<Vec<FunctionSpan>> {
    let registry = CompiledRegistry::compile_queries(supported_languages())?;
    scan_directory_with_registry(&registry, path, excludes)
}

/// Scan using queries that were already compiled for this run. Discovery commands
/// build one `CompiledRegistry` and share it between scanning and mutation.
pub fn scan_directory_with_registry(
    registry: &CompiledRegistry,
    path: &Path,
    excludes: &[String],
) -> anyhow::Result<Vec<FunctionSpan>> {
    let mut builder = ignore::WalkBuilder::new(path);
    if !excludes.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(path);
        for pat in excludes {
            overrides
                .add(&format!("!{pat}"))
                .with_context(|| format!("compiling exclude pattern {pat:?}"))?;
        }
        builder.overrides(
            overrides
                .build()
                .context("building exclude overrides")?,
        );
    }

    let mut spans = Vec::new();
    for result in builder.build() {
        let entry = result?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let file_path = entry.path();
        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(compiled) = registry.for_extension(ext) else {
            continue;
        };
        spans.extend(scan_file(file_path, compiled)?);
    }
    Ok(spans)
}

fn scan_file(path: &Path, compiled: &CompiledLanguage) -> anyhow::Result<Vec<FunctionSpan>> {
    // A function definition (named or anonymous) and its byte range. Anonymous
    // functions (closures, lambdas, arrow functions) get a synthetic name derived
    // from their start line so they are no longer dropped.
    struct Func<'a> {
        name: Option<String>,
        node: tree_sitter::Node<'a>,
        start: usize,
        end: usize,
    }

    let language = compiled.spec;
    let fn_query = &compiled.functions;
    let branch_query = &compiled.branches;

    let source =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let source_bytes = source.as_bytes();

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&compiled.ts_language)
        .with_context(|| format!("loading {} tree-sitter grammar", language.name()))?;
    let tree = parser
        .parse(&source, None)
        .with_context(|| format!("parsing {}", path.display()))?;

    let mut fn_cursor = tree_sitter::QueryCursor::new();
    let mut branch_cursor = tree_sitter::QueryCursor::new();

    // First pass: collect every function definition (named and anonymous) and its
    // byte range.
    let mut funcs: Vec<Func> = Vec::new();
    let mut matches = fn_cursor.matches(fn_query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        let mut name: Option<String> = None;
        let mut def_node: Option<tree_sitter::Node> = None;
        for capture in m.captures {
            match fn_query.capture_names()[capture.index as usize] {
                "fn.name" => {
                    name = capture
                        .node
                        .utf8_text(source_bytes)
                        .ok()
                        .map(std::string::ToString::to_string);
                }
                "fn.def" => {
                    def_node = Some(capture.node);
                }
                _ => {}
            }
        }
        if let Some(node) = def_node {
            funcs.push(Func {
                name,
                node,
                start: node.start_byte(),
                end: node.end_byte(),
            });
        }
    }

    // All function ranges, used to identify branches that belong to a strictly
    // nested function so they are not also charged to the enclosing function.
    let all_ranges: Vec<(usize, usize)> = funcs.iter().map(|f| (f.start, f.end)).collect();

    let mut spans = Vec::new();
    for func in funcs {
        // Ranges of functions strictly nested within this one.
        let nested: Vec<(usize, usize)> = all_ranges
            .iter()
            .copied()
            .filter(|(ns, ne)| {
                *ns >= func.start && *ne <= func.end && !(*ns == func.start && *ne == func.end)
            })
            .collect();

        let branch_count = count_branches(
            branch_query,
            &mut branch_cursor,
            func.node,
            source_bytes,
            &nested,
        );
        let cyclomatic = 1 + branch_count;

        let name = func
            .name
            .unwrap_or_else(|| format!("<anonymous>:{}", func.node.start_position().row + 1));

        spans.push(FunctionSpan {
            file: path.to_path_buf(),
            language: language.id,
            name,
            start_line: func.node.start_position().row + 1,
            end_line: func.node.end_position().row + 1,
            start_byte: func.start,
            end_byte: func.end,
            cyclomatic,
        });
    }
    Ok(spans)
}

fn count_branches(
    query: &tree_sitter::Query,
    cursor: &mut tree_sitter::QueryCursor,
    node: tree_sitter::Node,
    source: &[u8],
    nested: &[(usize, usize)],
) -> usize {
    let mut count = 0;
    let mut matches = cursor.matches(query, node, source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if query.capture_names()[capture.index as usize] == "branch" {
                let bs = capture.node.start_byte();
                if !nested.iter().any(|(ns, ne)| bs >= *ns && bs < *ne) {
                    count += 1;
                }
                break;
            }
        }
    }
    count
}
