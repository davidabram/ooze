use super::LanguageSpec;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/python/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/python/branches.scm");

// Python's mutator implementations (expands to `pub const MUTATORS`). The
// registry (`crate::mutate::registry`) aggregates this slice with the others'.
// Generic-syntax operators plus Python-specific ones (`is None` negation, `in`
// negation, truthiness, etc.); helper fns for the non-trivial ones live below.
mutators! {
    language: Python,
    id_prefix: "python",

    SwapBoolean {
        replace: |original| match original {
            "True" => Some("False".to_string()),
            "False" => Some("True".to_string()),
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
            "and" => Some("or".to_string()),
            "or" => Some("and".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Swap logical {original} -> {replacement}"),
    },
    IntegerZeroOne {
        replace: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap integer literal {original} -> {replacement}")
        },
    },
    // ── Python-specific operators ───────────────────────────────────────────
    IsNoneNegation {
        replace: |original| match original {
            "is" => Some("is not".to_string()),
            "is not" => Some("is".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Toggle None check {original} -> {replacement}")
        },
    },
    InNegation {
        replace: |original| match original {
            "in" => Some("not in".to_string()),
            "not in" => Some("in".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Toggle membership {original} -> {replacement}")
        },
    },
    TruthinessNegation {
        replace: negate_truthiness,
        describe: |original, replacement| {
            format!("Negate condition `{original}` -> `{replacement}`")
        },
    },
    LenZeroBoundary {
        replace: len_zero_boundary,
        describe: |original, replacement| {
            format!("Toggle emptiness check `{original}` -> `{replacement}`")
        },
    },
    DictGetDefaultRemoval {
        replace: dict_get_default_removal,
        describe: |original, replacement| {
            format!("Drop dict get default `{original}` -> `{replacement}`")
        },
    },
    ComprehensionFilterRemoval {
        replace: |_original| Some(String::new()),
        describe: |original, _replacement| {
            format!("Remove comprehension filter `{}`", original.trim())
        },
    },
    NoneReturn {
        replace: |original| {
            if original.trim() == "None" {
                None
            } else {
                Some("None".to_string())
            }
        },
        describe: |original, _replacement| format!("Return None instead of `{original}`"),
    },
    EmptyCollectionLiteral {
        replace: empty_collection_literal,
        describe: |original, replacement| {
            format!("Empty collection literal `{original}` -> `{replacement}`")
        },
    },
    // ── Reused cross-language operators ─────────────────────────────────────
    IteratorAnyAll {
        replace: |original| match original {
            "any" => Some("all".to_string()),
            "all" => Some("any".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap Python quantifier {original}(...) -> {replacement}(...)")
        },
    },
    ReturnBoolean {
        replace: |original| match original {
            "True" => Some("False".to_string()),
            "False" => Some("True".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip returned boolean {original} -> {replacement}")
        },
    },
    NegatePredicateMethod {
        replace: negate_python_predicate_call,
        describe: |original, replacement| {
            format!("Negate predicate call `{original}` -> `{replacement}`")
        },
    },
    MinMaxSwap {
        replace: |original| match original {
            "min" => Some("max".to_string()),
            "max" => Some("min".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Swap {original}(...) -> {replacement}(...)"),
    },
    SortedReverseFlip {
        replace: sorted_reverse_flip,
        describe: |original, replacement| {
            format!("Flip sorted ordering `{original}` -> `{replacement}`")
        },
    },
    DictGetToIndex {
        replace: dict_get_to_index,
        describe: |original, replacement| {
            format!("Replace get with indexing `{original}` -> `{replacement}`")
        },
    },
}

/// `sorted_reverse_flip`: flip a `sorted(...)` call's ordering. An existing
/// `reverse=True`/`reverse=False` keyword is toggled in place; a call with no
/// `reverse=` keyword gets `reverse=True` appended. Unrecognised `reverse=`
/// values (e.g. a variable) yield no mutant.
fn sorted_reverse_flip(original: &str) -> Option<String> {
    let trimmed = original.trim();
    let inner = trimmed.strip_prefix("sorted(")?.strip_suffix(')')?;
    let args = split_top_level_commas(inner);
    let mut out: Vec<String> = Vec::with_capacity(args.len() + 1);
    let mut flipped = false;
    for arg in &args {
        let a = arg.trim();
        if let Some(val) = a.strip_prefix("reverse")
            && let Some(v) = val.trim_start().strip_prefix('=')
        {
            let new = match v.trim() {
                "True" => "False",
                "False" => "True",
                _ => return None,
            };
            out.push(format!("reverse={new}"));
            flipped = true;
            continue;
        }
        out.push(a.to_string());
    }
    if !flipped {
        let mut base: Vec<String> = out.into_iter().filter(|s| !s.is_empty()).collect();
        if base.is_empty() {
            return None;
        }
        base.push("reverse=True".to_string());
        return Some(format!("sorted({})", base.join(", ")));
    }
    Some(format!("sorted({})", out.join(", ")))
}

/// `dict_get_to_index`: rewrite a single-argument `recv.get(key)` to `recv[key]`.
/// The query matches any `.get(...)` call; this confirms the method is `get` and
/// that there is exactly one top-level argument before subscripting. Two-argument
/// `.get(key, default)` is left to `dict_get_default_removal`.
fn dict_get_to_index(original: &str) -> Option<String> {
    let open = original.find('(')?;
    let head = &original[..open];
    let receiver = head.strip_suffix(".get")?.trim();
    if receiver.is_empty() || !original.ends_with(')') {
        return None;
    }
    let inner = &original[open + 1..original.len() - 1];
    let args = split_top_level_commas(inner);
    if args.len() != 1 {
        return None;
    }
    let key = args[0].trim();
    if key.is_empty() {
        return None;
    }
    Some(format!("{receiver}[{key}]"))
}

/// `negate_predicate_method`: flip a boolean-returning string predicate call by
/// wrapping it in `not (...)`. An existing leading `not` is unwrapped so the
/// mutation toggles cleanly; the parentheses keep operator precedence intact.
fn negate_python_predicate_call(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("not ") {
        let inner = rest.trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        Some(format!("not ({trimmed})"))
    }
}

/// `truthiness_negation`: flip an `if`/`while` condition. An existing leading
/// `not` is unwrapped; anything else is wrapped in `not (...)` so operator
/// precedence is preserved (e.g. `a and b` -> `not (a and b)`, not `not a and b`).
fn negate_truthiness(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if let Some(rest) = trimmed.strip_prefix("not ") {
        let inner = rest.trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        Some(format!("not ({trimmed})"))
    }
}

/// `len_zero_boundary`: rewrite the operator of a `len(...) <op> 0` comparison.
/// Only the `len(...)`-on-the-left, `0`-on-the-right canonical shape is handled;
/// other shapes (and non-`len` calls captured by the query) yield no mutant.
fn len_zero_boundary(original: &str) -> Option<String> {
    let trimmed = original.trim();
    if !trimmed.starts_with("len(") {
        return None;
    }
    for (from, to) in [("== 0", "!= 0"), ("!= 0", "== 0"), ("> 0", "== 0")] {
        if let Some(head) = trimmed.strip_suffix(from) {
            return Some(format!("{head}{to}"));
        }
    }
    None
}

/// `dict_get_default_removal`: turn `recv.get(key, default)` into
/// `recv.get(key)`. The query matches any two-or-more-arg method call, so this
/// confirms the method is `get` and that there are exactly two top-level
/// arguments before dropping the second.
fn dict_get_default_removal(original: &str) -> Option<String> {
    let open = original.find('(')?;
    let head = &original[..open];
    let method = head.rsplit('.').next().unwrap_or_default();
    if method != "get" || !original.ends_with(')') {
        return None;
    }
    let inner = &original[open + 1..original.len() - 1];
    let args = split_top_level_commas(inner);
    if args.len() != 2 {
        return None;
    }
    let first = args[0].trim();
    if first.is_empty() {
        return None;
    }
    Some(format!("{head}({first})"))
}

/// `empty_collection_literal`: replace a non-empty list/dict/set literal with its
/// empty form. Dict and set both open with `{`, so they are told apart by a
/// top-level `:` (or a `**` unpack, which is dict-only).
fn empty_collection_literal(original: &str) -> Option<String> {
    let trimmed = original.trim();
    match trimmed.as_bytes().first()? {
        b'[' => (trimmed != "[]").then(|| "[]".to_string()),
        b'{' => {
            if trimmed == "{}" {
                return None;
            }
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.trim_start().starts_with("**") || top_level_colon(inner) {
                Some("{}".to_string())
            } else {
                Some("set()".to_string())
            }
        }
        _ => None,
    }
}

/// Split `s` on commas that sit at bracket-nesting depth 0, ignoring commas
/// inside (), [], {} and string literals. Good enough for splitting call
/// arguments in source text.
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Whether `s` contains a `:` at bracket-nesting depth 0 (ignoring (), [], {} and
/// string literals). Used to tell a dict literal's body from a set literal's.
fn top_level_colon(s: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::Python,
    extensions: &["py"],
    language: || tree_sitter_python::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
