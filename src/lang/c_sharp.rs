use super::LanguageSpec;
use crate::lang::javascript::{
    empty_string_literal, negate_js_expression, remove_nullish_fallback, swap_ternary_arms,
};
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/c_sharp/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/c_sharp/branches.scm");

// C#'s mutation operator set: literal/operator swaps plus arithmetic,
// compound assignment, and unary mutations. The queries match syntax nodes
// (`boolean_literal`, `binary_expression` operators, `prefix_unary_expression`,
// `assignment_expression`), so `true` in a comment or `==` inside a string
// literal can never produce a candidate — except `string_empty_literal`, which
// intentionally targets regular `string_literal` nodes and is disabled by
// default. Null checks and conditional expressions are covered too:
// `nullish_coalescing_removal` drops the `??` fallback, `ternary_arm_swap` and
// `ternary_condition_negation` mutate `conditional_expression` nodes (never
// `if` statements). There is no separate null-check operator: `x == null` is a
// plain `binary_expression`, so `negate_equality` already produces the
// `x != null` mutant, and a null-specific operator would only create
// byte-identical duplicates. C#-specific operators cover the null-handling
// and overflow idioms: `null_forgiving_removal` (postfix `value!` -> `value`;
// the query anchors the `!` token so prefix logical not is never confused),
// `nullable_access_to_member_access` (`user?.Name` -> `user.Name`, disabled
// by default — it creates many null-reference mutants), `is_pattern_negation`
// (`x is P` <-> `x is not P`), `as_expression_to_direct_cast` (`x as T` ->
// `(T)x`, disabled by default — direct casts throw), `checked_unchecked_swap`
// (expression and block forms), `throw_expression_to_null` (throw
// *expressions* only, never throw statements; disabled by default), and
// `default_literal_to_null` (bare `default` only, never `default(T)`;
// disabled by default — invalid for non-nullable value types). Deliberately
// excluded for now: plain `=` and `%=` assignment, null insertion, LINQ/async
// rewrites — anything likely to produce non-compiling or noisy mutants.
mutators! {
    language: CSharp,
    id_prefix: "c_sharp",

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
    IntegerZeroOne {
        replace: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Replace integer {original} -> {replacement}"),
    },
    SwapArithmetic {
        replace: |original| match original {
            "+" => Some("-".to_string()),
            "-" => Some("+".to_string()),
            "*" => Some("/".to_string()),
            "/" | "%" => Some("*".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap arithmetic operator {original} -> {replacement}")
        },
    },
    SwapAssignment {
        replace: |original| match original {
            "+=" => Some("-=".to_string()),
            "-=" => Some("+=".to_string()),
            "*=" => Some("/=".to_string()),
            "/=" => Some("*=".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap assignment operator {original} -> {replacement}")
        },
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
    RemoveUnaryMinus {
        replace: |original| {
            let rest = original.strip_prefix('-')?.trim_start();
            if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            }
        },
        describe: |original, replacement| {
            format!("Remove unary minus `{original}` -> `{replacement}`")
        },
    },
    PlusToMinus {
        replace: |original| match original {
            "+" => Some("-".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Replace unary plus {original} -> {replacement}")
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
    StringEmptyLiteral {
        replace: empty_string_literal,
        describe: |original, replacement| {
            format!("Empty string literal `{original}` -> `{replacement}`")
        },
    },
    NullishCoalescingRemoval {
        replace: remove_nullish_fallback,
        describe: |original, replacement| {
            format!("Remove null-coalescing fallback `{original}` -> `{replacement}`")
        },
    },
    TernaryArmSwap {
        replace: swap_ternary_arms,
        describe: |original, replacement| {
            format!("Swap ternary arms `{original}` -> `{replacement}`")
        },
    },
    TernaryConditionNegation {
        replace: negate_js_expression,
        describe: |original, replacement| {
            format!("Negate ternary condition `{original}` -> `{replacement}`")
        },
    },
    NullForgivingRemoval {
        replace: remove_null_forgiving,
        describe: |original, replacement| {
            format!("Remove null-forgiving operator `{original}` -> `{replacement}`")
        },
    },
    NullableAccessToMemberAccess {
        replace: remove_nullable_access,
        describe: |original, replacement| {
            format!("Remove null-propagating access `{original}` -> `{replacement}`")
        },
    },
    IsPatternNegation {
        replace: negate_is_pattern,
        describe: |original, replacement| {
            format!("Negate is-pattern `{original}` -> `{replacement}`")
        },
    },
    AsExpressionToDirectCast {
        replace: as_to_direct_cast,
        describe: |original, replacement| {
            format!("Replace safe cast `{original}` -> `{replacement}`")
        },
    },
    CheckedUncheckedSwap {
        replace: |original| match original {
            "checked" => Some("unchecked".to_string()),
            "unchecked" => Some("checked".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap overflow context {original} -> {replacement}")
        },
    },
    ThrowExpressionToNull {
        replace: |original| {
            original
                .strip_prefix("throw")
                .map(|_| "null".to_string())
        },
        describe: |original, replacement| {
            format!("Replace throw expression `{original}` -> `{replacement}`")
        },
    },
    DefaultLiteralToNull {
        replace: |original| match original {
            // Only the bare literal; `default(T)` shares the node and is
            // deliberately skipped (invalid as null for value types).
            "default" => Some("null".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Replace default literal {original} -> {replacement}")
        },
    },
}

/// `null_forgiving_removal`: `value!` -> `value`. The query anchors the postfix
/// `!` token, so the text always ends with `!`; strip it and any space before
/// it. Returns `None` when nothing precedes the `!`.
fn remove_null_forgiving(original: &str) -> Option<String> {
    let rest = original.strip_suffix('!')?.trim_end();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// `nullable_access_to_member_access`: `user?.Name` -> `user.Name`,
/// `items?[0]` -> `items[0]`. Drops each `?` that starts a `?.` or `?[`
/// binding, scanning outside string/char literals so a `?.` inside a string
/// argument is never touched. Returns `None` when no binding is found.
fn remove_nullable_access(original: &str) -> Option<String> {
    let bytes = original.as_bytes();
    let mut quote: Option<u8> = None;
    let mut drop_at = Vec::new();
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
        } else {
            match c {
                b'"' | b'\'' => quote = Some(c),
                b'?' if matches!(bytes.get(i + 1), Some(b'.' | b'[')) => {
                    drop_at.push(i);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }
    if drop_at.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(original.len());
    let mut last = 0;
    for idx in drop_at {
        out.push_str(&original[last..idx]);
        last = idx + 1; // drop the `?`, keep the `.` or `[`
    }
    out.push_str(&original[last..]);
    Some(out)
}

/// Byte offset of the last top-level occurrence of keyword `kw` in `src`:
/// outside string/char literals, outside brackets, and bounded by
/// non-identifier characters on both sides so `island` or `Assert` never match.
fn find_top_level_keyword(src: &str, kw: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut found = None;
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
            b'"' | b'\'' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth == 0
                && src[i..].starts_with(kw)
                && (i == 0 || !is_ident(bytes[i - 1]))
                && !bytes.get(i + kw.len()).copied().is_some_and(is_ident) =>
            {
                found = Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    found
}

/// `is_pattern_negation`: `x is P` <-> `x is not P`. Splits on the top-level
/// `is` keyword and toggles a leading `not` on the pattern side. Returns
/// `None` when no top-level `is` is found or either side is empty.
fn negate_is_pattern(original: &str) -> Option<String> {
    let idx = find_top_level_keyword(original, "is")?;
    let left = original[..idx].trim_end();
    let pattern = original[idx + 2..].trim_start();
    if left.is_empty() || pattern.is_empty() {
        return None;
    }
    match pattern.strip_prefix("not") {
        Some(rest) if rest.starts_with(char::is_whitespace) => {
            Some(format!("{left} is {}", rest.trim_start()))
        }
        _ => Some(format!("{left} is not {pattern}")),
    }
}

/// `as_expression_to_direct_cast`: `value as T` -> `(T)value`. Splits on the
/// top-level `as` keyword and rebuilds a direct cast; compound operands are
/// parenthesized so precedence is preserved (`a + b as T` -> `(T)(a + b)`).
fn as_to_direct_cast(original: &str) -> Option<String> {
    let idx = find_top_level_keyword(original, "as")?;
    let expr = original[..idx].trim_end();
    let ty = original[idx + 2..].trim_start();
    if expr.is_empty() || ty.is_empty() {
        return None;
    }
    if expr.contains(char::is_whitespace) {
        Some(format!("({ty})({expr})"))
    } else {
        Some(format!("({ty}){expr}"))
    }
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: crate::core::Language::CSharp,
    extensions: &["cs"],
    language: || tree_sitter_c_sharp::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateExperimental,
    mutators: MUTATORS,
};
