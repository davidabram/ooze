use super::GrammarDef;
use crate::core::{Language, MutatorImpl, OperatorName};

const FUNCTIONS_QUERY: &str = include_str!("../../queries/python/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/python/branches.scm");

/// Python's mutator implementations. The registry (`crate::mutate::registry`)
/// aggregates this slice with the other languages' slices for discovery.
///
/// This is the generic-syntax MVP set: boolean literals, equality, comparison,
/// and logical operators. Python-specific operators (`is None` negation, `in`
/// negation, truthiness, etc.) can be layered on later as additional entries.
pub const MUTATORS: &[MutatorImpl] = &[
    MutatorImpl {
        id: "python.swap_boolean",
        operator: OperatorName::SwapBoolean,
        language: Language::Python,
        query: include_str!("../../queries/python/swap-boolean.scm"),
        replacement: |original| match original {
            "True" => Some("False".to_string()),
            "False" => Some("True".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap boolean literal {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.negate_equality",
        operator: OperatorName::NegateEquality,
        language: Language::Python,
        query: include_str!("../../queries/python/negate-equality.scm"),
        replacement: |original| match original {
            "==" => Some("!=".to_string()),
            "!=" => Some("==".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Negate equality {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.comparison_boundary",
        operator: OperatorName::ComparisonBoundary,
        language: Language::Python,
        query: include_str!("../../queries/python/comparison-boundary.scm"),
        replacement: |original| match original {
            "<" => Some("<=".to_string()),
            "<=" => Some("<".to_string()),
            ">" => Some(">=".to_string()),
            ">=" => Some(">".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Toggle comparison boundary {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.comparison_negation",
        operator: OperatorName::ComparisonNegation,
        language: Language::Python,
        query: include_str!("../../queries/python/comparison-negation.scm"),
        replacement: |original| match original {
            "<" => Some(">=".to_string()),
            "<=" => Some(">".to_string()),
            ">" => Some("<=".to_string()),
            ">=" => Some("<".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Negate comparison {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.swap_logical",
        operator: OperatorName::SwapLogical,
        language: Language::Python,
        query: include_str!("../../queries/python/swap-logical.scm"),
        replacement: |original| match original {
            "and" => Some("or".to_string()),
            "or" => Some("and".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap logical {original} -> {replacement}"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.integer_zero_one",
        operator: OperatorName::IntegerZeroOne,
        language: Language::Python,
        query: include_str!("../../queries/python/integer-zero-one.scm"),
        replacement: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap integer literal {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    // ── Python-specific operators ───────────────────────────────────────────
    MutatorImpl {
        id: "python.is_none_negation",
        operator: OperatorName::IsNoneNegation,
        language: Language::Python,
        query: include_str!("../../queries/python/is-none-negation.scm"),
        replacement: |original| match original {
            "is" => Some("is not".to_string()),
            "is not" => Some("is".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Toggle None check {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.in_negation",
        operator: OperatorName::InNegation,
        language: Language::Python,
        query: include_str!("../../queries/python/in-negation.scm"),
        replacement: |original| match original {
            "in" => Some("not in".to_string()),
            "not in" => Some("in".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Toggle membership {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.truthiness_negation",
        operator: OperatorName::TruthinessNegation,
        language: Language::Python,
        query: include_str!("../../queries/python/truthiness-negation.scm"),
        replacement: negate_truthiness,
        description: |original, replacement| {
            format!("Negate condition `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.len_zero_boundary",
        operator: OperatorName::LenZeroBoundary,
        language: Language::Python,
        query: include_str!("../../queries/python/len-zero-boundary.scm"),
        replacement: len_zero_boundary,
        description: |original, replacement| {
            format!("Toggle emptiness check `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.dict_get_default_removal",
        operator: OperatorName::DictGetDefaultRemoval,
        language: Language::Python,
        query: include_str!("../../queries/python/dict-get-default-removal.scm"),
        replacement: dict_get_default_removal,
        description: |original, replacement| {
            format!("Drop dict get default `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.comprehension_filter_removal",
        operator: OperatorName::ComprehensionFilterRemoval,
        language: Language::Python,
        query: include_str!("../../queries/python/comprehension-filter-removal.scm"),
        replacement: |_original| Some(String::new()),
        description: |original, _replacement| {
            format!("Remove comprehension filter `{}`", original.trim())
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.none_return",
        operator: OperatorName::NoneReturn,
        language: Language::Python,
        query: include_str!("../../queries/python/none-return.scm"),
        replacement: |original| {
            if original.trim() == "None" {
                None
            } else {
                Some("None".to_string())
            }
        },
        description: |original, _replacement| format!("Return None instead of `{original}`"),
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.empty_collection_literal",
        operator: OperatorName::EmptyCollectionLiteral,
        language: Language::Python,
        query: include_str!("../../queries/python/empty-collection-literal.scm"),
        replacement: empty_collection_literal,
        description: |original, replacement| {
            format!("Empty collection literal `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    // ── Reused cross-language operators ─────────────────────────────────────
    MutatorImpl {
        id: "python.iterator_any_all",
        operator: OperatorName::IteratorAnyAll,
        language: Language::Python,
        query: include_str!("../../queries/python/iterator-any-all.scm"),
        replacement: |original| match original {
            "any" => Some("all".to_string()),
            "all" => Some("any".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Swap Python quantifier {original}(...) -> {replacement}(...)")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.return_boolean",
        operator: OperatorName::ReturnBoolean,
        language: Language::Python,
        query: include_str!("../../queries/python/return-boolean.scm"),
        replacement: |original| match original {
            "True" => Some("False".to_string()),
            "False" => Some("True".to_string()),
            _ => None,
        },
        description: |original, replacement| {
            format!("Flip returned boolean {original} -> {replacement}")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.negate_predicate_method",
        operator: OperatorName::NegatePredicateMethod,
        language: Language::Python,
        query: include_str!("../../queries/python/negate-predicate-method.scm"),
        replacement: negate_python_predicate_call,
        description: |original, replacement| {
            format!("Negate predicate call `{original}` -> `{replacement}`")
        },
        default_enabled_override: None,
    },
    MutatorImpl {
        id: "python.min_max_swap",
        operator: OperatorName::MinMaxSwap,
        language: Language::Python,
        query: include_str!("../../queries/python/min-max-swap.scm"),
        replacement: |original| match original {
            "min" => Some("max".to_string()),
            "max" => Some("min".to_string()),
            _ => None,
        },
        description: |original, replacement| format!("Swap {original}(...) -> {replacement}(...)"),
        default_enabled_override: None,
    },
];

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

pub const GRAMMAR: GrammarDef = GrammarDef {
    id: crate::core::Language::Python,
    extensions: &["py"],
    language: || tree_sitter_python::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
};
