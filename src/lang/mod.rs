use std::path::Path;

use anyhow::Context;
use streaming_iterator::StreamingIterator;

use crate::core::FunctionSpan;

const RUST_FUNCTIONS_QUERY: &str = include_str!("../../queries/rust/functions.scm");
const RUST_BRANCHES_QUERY: &str = include_str!("../../queries/rust/branches.scm");

pub fn scan_directory(path: &Path) -> anyhow::Result<Vec<FunctionSpan>> {
    let mut spans = Vec::new();
    for result in ignore::WalkBuilder::new(path).build() {
        let entry = result?;
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }
        let file_path = entry.path();
        if file_path.extension().map_or(true, |ext| ext != "rs") {
            continue;
        }
        spans.extend(scan_file(file_path)?);
    }
    Ok(spans)
}

fn scan_file(path: &Path) -> anyhow::Result<Vec<FunctionSpan>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let source_bytes = source.as_bytes();

    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .with_context(|| "loading Rust tree-sitter grammar")?;
    let tree = parser
        .parse(&source, None)
        .with_context(|| format!("parsing {}", path.display()))?;

    let fn_query = tree_sitter::Query::new(&language, RUST_FUNCTIONS_QUERY)
        .with_context(|| "compiling function query")?;
    let branch_query = tree_sitter::Query::new(&language, RUST_BRANCHES_QUERY)
        .with_context(|| "compiling branch query")?;
    let mut fn_cursor = tree_sitter::QueryCursor::new();
    let mut branch_cursor = tree_sitter::QueryCursor::new();

    let mut spans = Vec::new();
    let mut matches = fn_cursor.matches(&fn_query, tree.root_node(), source_bytes);
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
        if let (Some(name), Some(def_node)) = (name, def_node) {
            let branch_count =
                count_branches(&branch_query, &mut branch_cursor, def_node, source_bytes);
            let cyclomatic = 1 + branch_count;
            spans.push(FunctionSpan {
                file: path.to_path_buf(),
                language: "rust".to_string(),
                name,
                start_line: def_node.start_position().row + 1,
                end_line: def_node.end_position().row + 1,
                start_byte: def_node.start_byte(),
                end_byte: def_node.end_byte(),
                cyclomatic,
            });
        }
    }
    Ok(spans)
}

fn count_branches(
    query: &tree_sitter::Query,
    cursor: &mut tree_sitter::QueryCursor,
    node: tree_sitter::Node,
    source: &[u8],
) -> usize {
    let mut count = 0;
    let mut matches = cursor.matches(query, node, source);
    while matches.next().is_some() {
        count += 1;
    }
    count
}
