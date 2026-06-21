use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::MutationCandidate;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CandidateSkip {
    pub rule: &'static str,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkippedCandidate {
    #[serde(flatten)]
    pub candidate: MutationCandidate,
    pub skip_rule: &'static str,
    pub skip_reason: String,
}

const RUST_ASSERT_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
];

const RUST_PANIC_MACROS: &[&str] = &["panic", "unreachable", "todo", "unimplemented"];

struct FileContext {
    is_test: bool,
    is_generated: bool,
    macros: Vec<MacroRange>,
}

struct MacroRange {
    name: String,
    inner_start: usize,
    inner_end: usize,
}

fn is_test_path(path: &Path) -> bool {
    for c in path.components() {
        let seg = c.as_os_str().to_string_lossy();
        if matches!(seg.as_ref(), "tests" | "__tests__" | "spec" | "specs") {
            return true;
        }
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // Extract stem (e.g. "foo.test" from "foo.test.ts", "foo_test" from "foo_test.go")
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    stem == "tests"
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || stem.ends_with("_specs")
        || stem.ends_with(".test")
        || stem.ends_with(".spec")
        || stem.starts_with("test_")
        || stem.starts_with("spec_")
        || name == "conftest.py"
}

fn detect_generated(source: &str) -> bool {
    for line in source.lines().take(15) {
        let trimmed = line.trim_start();
        let comment = trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.starts_with('#');
        if !comment {
            continue;
        }
        if trimmed.contains("@generated") || trimmed.contains("DO NOT EDIT") {
            return true;
        }
    }
    false
}

fn find_macro_ranges(source: &str) -> Vec<MacroRange> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let n = bytes.len();

    while i < n {
        let c = bytes[i];

        if c == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if c == b'/' && i + 1 < n {
            if bytes[i + 1] == b'/' {
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                i = skip_block_comment(bytes, i + 2);
                continue;
            }
        }

        if is_ident_start(c) {
            let start = i;
            while i < n && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let name = &source[start..i];
            if i < n && bytes[i] == b'!' {
                let mut j = i + 1;
                while j < n && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n {
                    let (open, close) = match bytes[j] {
                        b'(' => (b'(', b')'),
                        b'[' => (b'[', b']'),
                        b'{' => (b'{', b'}'),
                        _ => {
                            continue;
                        }
                    };
                    let inner_start = j + 1;
                    if let Some(inner_end) = find_matching(bytes, inner_start, open, close) {
                        out.push(MacroRange {
                            name: name.to_string(),
                            inner_start,
                            inner_end,
                        });
                        i = inner_end + 1;
                        continue;
                    }
                }
            }
            continue;
        }

        i += 1;
    }

    out
}

fn is_ident_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

fn skip_string(bytes: &[u8], start: usize) -> usize {
    let n = bytes.len();
    let mut i = start + 1;
    while i < n {
        match bytes[i] {
            b'\\' if i + 1 < n => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    n
}

fn skip_block_comment(bytes: &[u8], start: usize) -> usize {
    let n = bytes.len();
    let mut i = start;
    let mut depth: i32 = 1;
    while i + 1 < n && depth > 0 {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            depth -= 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    i
}

fn find_matching(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let n = bytes.len();
    let mut depth = 1i32;
    let mut i = start;
    while i < n {
        match bytes[i] {
            b'"' => i = skip_string(bytes, i),
            b'/' if i + 1 < n && bytes[i + 1] == b'/' => {
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
                i = skip_block_comment(bytes, i + 2);
            }
            c if c == open => {
                depth += 1;
                i += 1;
            }
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn load_context(path: &Path) -> Option<FileContext> {
    let source = std::fs::read_to_string(path).ok()?;
    Some(FileContext {
        is_test: is_test_path(path),
        is_generated: detect_generated(&source),
        macros: find_macro_ranges(&source),
    })
}

fn classify(candidate: &MutationCandidate, ctx: &FileContext) -> Option<CandidateSkip> {
    if ctx.is_generated {
        return Some(CandidateSkip {
            rule: "generated_file",
            reason: "Source marked @generated or DO NOT EDIT".to_string(),
        });
    }
    if ctx.is_test {
        return Some(CandidateSkip {
            rule: "test_file",
            reason: "Candidate is in a test-only file".to_string(),
        });
    }

    let bs = candidate.start_byte;
    let inside = ctx
        .macros
        .iter()
        .filter(|m| bs >= m.inner_start && bs < m.inner_end)
        .min_by_key(|m| m.inner_end - m.inner_start);

    if let Some(m) = inside {
        if RUST_ASSERT_MACROS.iter().any(|n| *n == m.name) {
            return Some(CandidateSkip {
                rule: "assertion_macro",
                reason: format!("Candidate is inside {}! argument", m.name),
            });
        }
        if RUST_PANIC_MACROS.iter().any(|n| *n == m.name) {
            return Some(CandidateSkip {
                rule: "panic_macro",
                reason: format!("Candidate is inside {}! argument", m.name),
            });
        }
    }

    None
}

pub fn partition(
    candidates: Vec<MutationCandidate>,
) -> (Vec<MutationCandidate>, Vec<SkippedCandidate>) {
    let mut cache: HashMap<PathBuf, Option<FileContext>> = HashMap::new();
    let mut kept = Vec::new();
    let mut skipped = Vec::new();

    for c in candidates {
        let ctx = cache
            .entry(c.file.clone())
            .or_insert_with(|| load_context(&c.file));

        let decision = match ctx {
            Some(ctx) => classify(&c, ctx),
            None => None,
        };

        match decision {
            Some(skip) => skipped.push(SkippedCandidate {
                candidate: c,
                skip_rule: skip.rule,
                skip_reason: skip.reason,
            }),
            None => kept.push(c),
        }
    }

    (kept, skipped)
}
