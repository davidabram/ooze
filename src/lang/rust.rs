use super::LanguageSpec;
use crate::core::Language;
use crate::lang::mutators;

const FUNCTIONS_QUERY: &str = include_str!("../../queries/rust/functions.scm");
const BRANCHES_QUERY: &str = include_str!("../../queries/rust/branches.scm");

// Rust's mutator implementations (expands to `pub const MUTATORS`). The registry
// (`crate::mutate::registry`) aggregates this slice with the other languages'.
mutators! {
    language: Rust,
    id_prefix: "rust",

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
    IntegerZeroOne {
        replace: |original| match original {
            "0" => Some("1".to_string()),
            "1" => Some("0".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Replace integer {original} -> {replacement}"),
    },
    RangeInclusiveExclusive {
        replace: |original| match original {
            ".." => Some("..=".to_string()),
            "..=" => Some("..".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Toggle range bound {original} -> {replacement}")
        },
    },
    SwapPredicateMethod {
        replace: |original| match original {
            "is_some" => Some("is_none".to_string()),
            "is_none" => Some("is_some".to_string()),
            "is_ok" => Some("is_err".to_string()),
            "is_err" => Some("is_ok".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap predicate method {original}() -> {replacement}()")
        },
    },
    NegatePredicateMethod {
        // The query only matches bool-returning predicate calls, so wrapping the
        // whole call expression in `!` is always type-correct.
        replace: |original| Some(format!("!{original}")),
        describe: |original, replacement| {
            format!("Negate predicate method `{original}` -> `{replacement}`")
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
            "any" => Some("all".to_string()),
            "all" => Some("any".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap iterator quantifier {original}(...) -> {replacement}(...)")
        },
    },
    MatchBoolPattern {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip match-arm boolean pattern {original} -> {replacement}")
        },
    },
    OkErrBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip wrapped boolean {original} -> {replacement}")
        },
    },
    SomeBoolean {
        replace: |original| match original {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Flip wrapped boolean {original} -> {replacement}")
        },
    },
    OptionSomeNone {
        // The whole `Some(value)` call expression is the @target; replace it
        // with `None`. The query already scopes matches to the `Some` ctor.
        replace: |original| {
            if original.starts_with("Some") {
                Some("None".to_string())
            } else {
                None
            }
        },
        describe: |original, _replacement| format!("Replace `{original}` with `None`"),
    },
    RemoveTry {
        replace: |original| original.strip_suffix('?').map(str::to_string),
        describe: |original, replacement| {
            format!("Remove `?` propagation `{original}` -> `{replacement}`")
        },
    },
    UnwrapToUnwrapOrDefault {
        replace: |original| match original {
            "unwrap" => Some("unwrap_or_default".to_string()),
            _ => None,
        },
        describe: |_original, _replacement| {
            "Replace `unwrap()` with `unwrap_or_default()`".to_string()
        },
    },
    MinMaxSwap {
        replace: |original| match original {
            "min" => Some("max".to_string()),
            "max" => Some("min".to_string()),
            _ => None,
        },
        describe: |original, replacement| format!("Swap {original} -> {replacement}"),
    },
    MatchWildcardToPanic {
        replace: |_original| Some("panic!(\"ooze mutant\")".to_string()),
        describe: |original, _replacement| {
            format!("Replace wildcard arm value `{original}` with a panic")
        },
    },
    EmptyVecMacro {
        // The query already scopes matches to the `vec!` macro; replace the whole
        // invocation with an empty one.
        replace: |original| {
            if original.starts_with("vec!") {
                Some("vec![]".to_string())
            } else {
                None
            }
        },
        describe: |original, _replacement| format!("Empty `{original}` -> `vec![]`"),
    },
    SaturatingCheckedSwap {
        replace: |original| match original {
            "checked_add" => Some("saturating_add".to_string()),
            "saturating_add" => Some("checked_add".to_string()),
            "checked_sub" => Some("saturating_sub".to_string()),
            "saturating_sub" => Some("checked_sub".to_string()),
            _ => None,
        },
        describe: |original, replacement| {
            format!("Swap overflow handling {original}(...) -> {replacement}(...)")
        },
    },
    ExpectToUnwrapOrDefault {
        // The @target is the whole `recv.expect(msg)` call. Split off the receiver
        // at `.expect(` and drop the message so the result is `recv.unwrap_or_default()`.
        replace: |original| {
            let receiver = original.split(".expect(").next()?;
            if receiver == original {
                return None;
            }
            Some(format!("{receiver}.unwrap_or_default()"))
        },
        describe: |_original, _replacement| {
            "Replace `expect(..)` with `unwrap_or_default()`".to_string()
        },
    },
}

pub const SPEC: LanguageSpec = LanguageSpec {
    id: Language::Rust,
    extensions: &["rs"],
    language: || tree_sitter_rust::LANGUAGE.into(),
    functions_query: FUNCTIONS_QUERY,
    branches_query: BRANCHES_QUERY,
    support: crate::core::SupportLevel::MutateStable,
    mutators: MUTATORS,
};
