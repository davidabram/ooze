use std::collections::BTreeSet;

use crate::core::{MutationCandidate, OperatorName};

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorFilterMode {
    RegistryDefaults,
    Explicit,
}

#[derive(Debug, Clone)]
pub struct OperatorFilter {
    pub mode: OperatorFilterMode,
    pub include: BTreeSet<OperatorName>,
    pub exclude: BTreeSet<OperatorName>,
}

impl OperatorFilter {
    pub fn from_cli(operators: &[OperatorName], exclude_operators: &[OperatorName]) -> Self {
        let exclude: BTreeSet<OperatorName> = exclude_operators.iter().copied().collect();
        if operators.is_empty() {
            let include = OperatorName::ALL
                .iter()
                .copied()
                .filter(|op| op.info().default_enabled)
                .collect();
            Self {
                mode: OperatorFilterMode::RegistryDefaults,
                include,
                exclude,
            }
        } else {
            Self {
                mode: OperatorFilterMode::Explicit,
                include: operators.iter().copied().collect(),
                exclude,
            }
        }
    }

    pub fn allows(&self, op: OperatorName) -> bool {
        if self.exclude.contains(&op) {
            return false;
        }
        self.include.contains(&op)
    }

    pub fn apply(&self, candidates: Vec<MutationCandidate>) -> Vec<MutationCandidate> {
        candidates
            .into_iter()
            .filter(|c| self.allows(c.operator))
            .collect()
    }

    pub fn included_after_excludes(&self) -> Vec<OperatorName> {
        self.include
            .iter()
            .copied()
            .filter(|op| !self.exclude.contains(op))
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OperatorFilterReport {
    pub mode: OperatorFilterMode,
    pub included: Vec<OperatorName>,
    pub excluded: Vec<OperatorName>,
}

impl From<&OperatorFilter> for OperatorFilterReport {
    fn from(f: &OperatorFilter) -> Self {
        Self {
            mode: f.mode,
            included: f.included_after_excludes(),
            excluded: f.exclude.iter().copied().collect(),
        }
    }
}
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use crate::lang::Language;

pub fn discover_mutants(
    functions: &[crate::core::FunctionSpan],
    languages: &[Box<dyn Language>],
) -> Result<Vec<MutationCandidate>> {
    let mut candidates = Vec::new();

    for function in functions {
        let Some(lang) = languages.iter().find(|l| l.name() == function.language) else {
            continue;
        };

        let operators = lang.mutation_operators();
        if operators.is_empty() {
            continue;
        }

        let source = std::fs::read_to_string(&function.file)
            .with_context(|| format!("reading {}", function.file.display()))?;

        let ts_lang = lang.tree_sitter_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts_lang)
            .with_context(|| format!("loading {} grammar", lang.name()))?;

        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("parsing {}", function.file.display()))?;

        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let function_node = find_node_by_byte_range(root, function.start_byte, function.end_byte);

        let Some(function_node) = function_node else {
            continue;
        };

        for op in operators {
            let query = Query::new(&ts_lang, op.query)
                .with_context(|| format!("compiling {} mutation query", op.name))?;

            let Some(target_idx) = query.capture_index_for_name("target") else {
                continue;
            };

            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, function_node, source_bytes);

            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if capture.index != target_idx {
                        continue;
                    }

                    let node = capture.node;
                    let original = node_text(node, source_bytes);
                    let Some(replacement) = (op.replacement)(&original) else {
                        continue;
                    };

                    let candidate_file = normalize_path(&function.file);

                    candidates.push(MutationCandidate {
                        id: format!(
                            "{}:{}:{}:{}",
                            candidate_file.display(),
                            node.start_position().row + 1,
                            node.start_position().column,
                            op.name,
                        ),
                        file: function.file.clone(),
                        language: function.language.clone(),
                        function: function.name.clone(),
                        line: node.start_position().row + 1,
                        column: node.start_position().column,
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        operator: op.name,
                        description: (op.description)(&original, &replacement),
                        original,
                        replacement,
                    });
                }
            }
        }
    }

    Ok(candidates)
}

fn find_node_by_byte_range(root: Node, start_byte: usize, end_byte: usize) -> Option<Node> {
    fn visit(node: Node, start_byte: usize, end_byte: usize) -> Option<Node> {
        if node.start_byte() == start_byte && node.end_byte() == end_byte {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.start_byte() <= start_byte
                && child.end_byte() >= end_byte
                && let Some(found) = visit(child, start_byte, end_byte)
            {
                return Some(found);
            }
        }

        None
    }

    visit(root, start_byte, end_byte)
}

fn normalize_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with("./") {
        PathBuf::from(&path_str[2..])
    } else {
        path.to_path_buf()
    }
}

fn node_text(node: Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("<invalid utf8>")
        .to_string()
}
