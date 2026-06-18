use std::path::Path;

use anyhow::Context;
use streaming_iterator::StreamingIterator;

use crate::core::{FunctionSpan, MutationOperator};

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

pub trait Language {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    fn functions_query(&self) -> &'static str;
    fn branches_query(&self) -> &'static str;
    fn mutation_operators(&self) -> &'static [MutationOperator] {
        &[]
    }
}

pub fn supported_languages() -> Vec<Box<dyn Language>> {
    vec![
        Box::new(bash::Bash),
        Box::new(javascript::JavaScript),
        Box::new(typescript::TypeScript),
        Box::new(python::Python),
        Box::new(java::Java),
        Box::new(c_sharp::CSharp),
        Box::new(cpp::Cpp),
        Box::new(c::C),
        Box::new(dart::Dart),
        Box::new(elixir::Elixir),
        Box::new(erlang::Erlang),
        Box::new(gleam::Gleam),
        Box::new(go::Go),
        Box::new(haskell::Haskell),
        Box::new(julia::Julia),
        Box::new(lua::Lua),
        Box::new(ocaml::Ocaml),
        Box::new(rust::Rust),
        Box::new(ruby::Ruby),
        Box::new(php::Php),
        Box::new(scala::Scala),
        Box::new(swift::Swift),
        Box::new(zig::Zig),
    ]
}

struct Compiled {
    language: Box<dyn Language>,
    functions: tree_sitter::Query,
    branches: tree_sitter::Query,
}

pub fn scan_directory(path: &Path) -> anyhow::Result<Vec<FunctionSpan>> {
    scan_directory_with_excludes(path, &[])
}

pub fn scan_directory_with_excludes(
    path: &Path,
    excludes: &[String],
) -> anyhow::Result<Vec<FunctionSpan>> {
    let languages = supported_languages();
    let mut compiled = Vec::with_capacity(languages.len());
    for language in languages {
        let ts_lang = language.tree_sitter_language();
        let functions = tree_sitter::Query::new(&ts_lang, language.functions_query())
            .with_context(|| format!("compiling {} function query", language.name()))?;
        let branches = tree_sitter::Query::new(&ts_lang, language.branches_query())
            .with_context(|| format!("compiling {} branch query", language.name()))?;
        compiled.push(Compiled {
            language,
            functions,
            branches,
        });
    }

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
        let ext = match file_path.extension().and_then(|e| e.to_str()) {
            Some(ext) => ext,
            None => continue,
        };
        let Some(compiled) = compiled
            .iter()
            .find(|c| c.language.extensions().contains(&ext))
        else {
            continue;
        };
        spans.extend(scan_file(
            file_path,
            compiled.language.as_ref(),
            &compiled.functions,
            &compiled.branches,
        )?);
    }
    Ok(spans)
}

fn scan_file(
    path: &Path,
    language: &dyn Language,
    fn_query: &tree_sitter::Query,
    branch_query: &tree_sitter::Query,
) -> anyhow::Result<Vec<FunctionSpan>> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let source_bytes = source.as_bytes();

    let ts_lang = language.tree_sitter_language();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_lang)
        .with_context(|| format!("loading {} tree-sitter grammar", language.name()))?;
    let tree = parser
        .parse(&source, None)
        .with_context(|| format!("parsing {}", path.display()))?;

    let mut fn_cursor = tree_sitter::QueryCursor::new();
    let mut branch_cursor = tree_sitter::QueryCursor::new();

    // First pass: collect every function definition (named and anonymous) and its
    // byte range. Anonymous functions (closures, lambdas, arrow functions) get a
    // synthetic name derived from their start line so they are no longer dropped.
    struct Func<'a> {
        name: Option<String>,
        node: tree_sitter::Node<'a>,
        start: usize,
        end: usize,
    }

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
                        .map(|s| s.to_string());
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
            language: language.name().to_string(),
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
