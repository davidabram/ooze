use super::GrammarDef;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str =
    include_str!("../../queries/javascript/functions.scm");
const BRANCHES_QUERY: &str =
    include_str!("../../queries/javascript/branches.scm");

// JavaScript's mutator implementations (expands to `pub const MUTATORS`). The
// registry (`crate::mutate::registry`) aggregates this slice with the others'.
mutators! {
    language: JavaScript,
    id_prefix: "javascript",

    SwapBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap boolean literal {original} -> {replacement}")
        },
    },
    NegateEquality {
        replace: |original| match original {
            "==" => Some("!=".to_string()),
            "!=" => Some("==".to_string()),
            "===" => Some("!==".to_string()),
            "!==" => Some("===".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Negate equality {original} -> {replacement}")
        },
    },
    ComparisonBoundary {
        replace: |original| match original {
            "<" => Some("<=".to_string()),
            "<=" => Some("<".to_string()),
            ">" => Some(">=".to_string()),
            ">=" => Some(">".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Toggle comparison boundary {original} -> {replacement}")
        },
    },
    ComparisonNegation {
        replace: |original| match original {
            "<" => Some(">=".to_string()),
            "<=" => Some(">".to_string()),
            ">" => Some("<=".to_string()),
            ">=" => Some("<".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Negate comparison {original} -> {replacement}")
        },
    },
    SwapLogical {
        replace: |original| match original {
            "&&" => Some("||".to_string()),
            "||" => Some("&&".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Swap logical {original} -> {replacement}"),
    },
    RemoveNot {
        replace: |original| {
            let rest = original.strip_prefix('!')?.trim_start();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        },
        describe: |original, replacement| {
            format!("Remove negation `{original}` -> `{replacement}`")
        },
    },
    ReturnBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip returned boolean {original} -> {replacement}")
        },
    },
    IteratorAnyAll {
        replace: |original| match original {
            "some" => Some("every".to_string()),
            "every" => Some("some".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap iterator quantifier {original}(...) -> {replacement}(...)")
        },
    },
    StringBoundaryMethodSwap {
        replace: |original| match original {
            "startsWith" => Some("endsWith".to_string()),
            "endsWith" => Some("startsWith".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap string boundary method {original}(...) -> {replacement}(...)")
        },
    },
    IncludesNegation {
        replace: negate_js_expression,
        describe: |original, replacement| {
            format!("Negate membership `{original}` -> `{replacement}`")
        },
    },
    NullishCoalescingRemoval {
        replace: remove_nullish_fallback,
        describe: |original, replacement| {
            format!("Remove nullish fallback `{original}` -> `{replacement}`")
        },
    },
    OptionalChainingRemoval {
        replace: remove_optional_chaining,
        describe: |original, replacement| {
            format!("Remove optional chaining `{original}` -> `{replacement}`")
        },
    },
    TernaryArmSwap {
        replace: swap_ternary_arms,
        describe: |original, replacement| {
            format!("Swap ternary arms `{original}` -> `{replacement}`")
        },
    },
    ArrayEmptyLiteral {
        replace: empty_array_literal,
        describe: |original, replacement| {
            format!("Empty array literal `{original}` -> `{replacement}`")
        },
    },
    ObjectEmptyLiteral {
        replace: empty_object_literal,
        describe: |original, replacement| {
            format!("Empty object literal `{original}` -> `{replacement}`")
        },
    },
    StringEmptyLiteral {
        replace: empty_string_literal,
        describe: |original, replacement| {
            format!("Empty string literal `{original}` -> `{replacement}`")
        },
    },
    AwaitRemoval {
        replace: remove_await,
        describe: |original, replacement| {
            format!("Remove await `{original}` -> `{replacement}`")
        },
    },
}

/// `includes_negation`: flip an `includes` membership predicate by wrapping it in
/// `!(...)`. An existing leading `!` is unwrapped so the mutation toggles
/// cleanly; the parentheses keep operator precedence intact.
pub(crate) fn negate_js_expression(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('!') {
        let inner = rest.trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        Some(format!("!({trimmed})"))
    }
}

/// Find the byte offset of the first occurrence of `op` at the top level of
/// `src` — i.e. not nested inside `()`, `[]`, `{}`, and not inside a string or
/// template literal. Returns `None` if `op` does not appear at the top level.
/// Used by the nullish-coalescing helper to split on `??` without tripping on a
/// `??` that lives inside a nested expression or string.
fn find_top_level(src: &str, op: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth == 0 && src[i..].starts_with(op) => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// `nullish_coalescing_removal`: `a ?? b` -> `a`. Splits on the first top-level
/// `??` and keeps the left operand, dropping the fallback. Returns `None` when
/// there is no top-level `??` or the left side is empty.
pub(crate) fn remove_nullish_fallback(original: &str) -> Option<String> {
    let idx = find_top_level(original, "??")?;
    let left = original[..idx].trim();
    if left.is_empty() {
        None
    } else {
        Some(left.to_string())
    }
}

/// `optional_chaining_removal`: `a?.b` -> `a.b`, `fn?.()` -> `fn()`. Rewrites the
/// optional-chain token to its plain form. The `?.(` case is handled before
/// `?.` so an optional call collapses to a plain call. Returns `None` when the
/// expression contains no `?.`.
pub(crate) fn remove_optional_chaining(original: &str) -> Option<String> {
    if !original.contains("?.") {
        return None;
    }
    let out = original.replace("?.(", "(").replace("?.", ".");
    Some(out)
}

/// `ternary_arm_swap`: `cond ? a : b` -> `cond ? b : a`. Splits only the
/// top-level ternary `?` and its matching top-level `:`, skipping `??` and `?.`
/// tokens and ignoring `?`/`:` nested in brackets or strings. Returns `None` for
/// ambiguous input where a matching `?`/`:` pair cannot be located.
pub(crate) fn swap_ternary_arms(original: &str) -> Option<String> {
    let bytes = original.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut q_idx: Option<usize> = None;
    let mut c_idx: Option<usize> = None;
    let mut ternary_depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'?' if depth == 0 => {
                // `??` and `?.` are not ternary question marks.
                if matches!(bytes.get(i + 1), Some(b'?' | b'.')) {
                    i += 2;
                    continue;
                }
                if q_idx.is_none() {
                    q_idx = Some(i);
                }
                ternary_depth += 1;
            }
            b':' if depth == 0 && q_idx.is_some() => {
                ternary_depth -= 1;
                if ternary_depth == 0 {
                    c_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let (q, colon) = (q_idx?, c_idx?);
    let cond = &original[..q];
    let true_arm = original[q + 1..colon].trim();
    let false_arm = original[colon + 1..].trim();
    if true_arm.is_empty() || false_arm.is_empty() {
        return None;
    }
    Some(format!("{cond}? {false_arm} : {true_arm}"))
}

/// `array_empty_literal`: `[a, b]` -> `[]`. Only fires when the text is a bracket
/// literal that is not already empty.
pub(crate) fn empty_array_literal(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed != "[]" {
        Some("[]".to_string())
    } else {
        None
    }
}

/// `object_empty_literal`: `{ a: 1 }` -> `{}`. Only fires when the text is a brace
/// literal that is not already empty.
pub(crate) fn empty_object_literal(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed != "{}" {
        Some("{}".to_string())
    } else {
        None
    }
}

/// `string_empty_literal`: replace a non-empty string with an empty one of the
/// same kind — double-quoted, single-quoted, or a backtick template. Skips
/// already-empty strings and template strings that contain a `${...}`
/// interpolation (where emptying would drop a runtime expression).
pub(crate) fn empty_string_literal(original: &str) -> Option<String> {
    let bytes = original.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    match first {
        b'"' if last == b'"' => (original != "\"\"").then(|| "\"\"".to_string()),
        b'\'' if last == b'\'' => (original != "''").then(|| "''".to_string()),
        b'`' if last == b'`' => {
            if original.contains("${") || original == "``" {
                None
            } else {
                Some("``".to_string())
            }
        }
        _ => None,
    }
}

/// `await_removal`: `await x` -> `x`. Strips the leading `await ` keyword.
/// Returns `None` when the expression does not start with `await` followed by
/// whitespace.
pub(crate) fn remove_await(original: &str) -> Option<String> {
    let rest = original.trim_start().strip_prefix("await")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::JavaScript,
    extensions: &["js", "jsx", "mjs", "cjs"],
    language: || tree_sitter_javascript::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
