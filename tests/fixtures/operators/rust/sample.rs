// Mutation-operator discovery fixture for Rust.
//
// Each function isolates one obvious mutation target so the discovery tests can
// assert the produced candidates by compact shape (function, operator, original,
// replacement) without caring about line/column. Keep these boring and
// unambiguous. Where two operators legitimately fire on the same node, that
// overlap is called out in a comment so the expected set stays explainable.

fn swap_boolean() -> bool {
    // `true` is a plain boolean literal (not in return position) -> swap_boolean only.
    let enabled = true;
    enabled
}

fn negate_equality(a: i32, b: i32) -> bool {
    a == b
}

fn compare(a: i32, b: i32) -> bool {
    // The `<` node drives both comparison_boundary (< -> <=) and
    // comparison_negation (< -> >=).
    a < b
}

fn swap_logical(x: bool, y: bool) -> bool {
    x && y
}

fn remove_not(flag: bool) -> bool {
    !flag
}

fn integer_zero_one() -> i32 {
    // integer_zero_one is default-disabled; only discovered when explicitly enabled.
    let n = 0;
    n
}

fn range_bound(n: usize, items: &[i32]) -> i32 {
    // `0..n` exercises range_inclusive_exclusive (.. -> ..=). The `0` literal also
    // feeds integer_zero_one, which stays dormant under default operators.
    let mut total = 0;
    for i in 0..n {
        total += items[i];
    }
    total
}

fn swap_predicate_method(opt: Option<i32>) -> bool {
    opt.is_some()
}

fn negate_predicate_method(s: &str) -> bool {
    s.is_empty()
}

fn return_boolean() -> bool {
    // The literal in `return true` drives return_boolean (true -> false) and also
    // swap_boolean, which matches every boolean literal.
    return true;
}

fn iterator_any_all(xs: &[i32]) -> bool {
    // `any` drives iterator_any_all (any -> all). The `is_positive` method name is
    // outside every predicate operator's curated set, so nothing else fires.
    xs.iter().any(|n| n.is_positive())
}

fn match_bool_pattern(flag: bool) -> i32 {
    // `true`/`false` patterns drive match_bool_pattern, and swap_boolean matches
    // each boolean literal too. The 10/20 arms avoid integer_zero_one.
    match flag {
        true => 10,
        false => 20,
    }
}

fn ok_err_boolean() -> Result<bool, ()> {
    // `Ok(true)` drives ok_err_boolean (true -> false) and swap_boolean.
    Ok(true)
}

fn some_boolean() -> Option<bool> {
    // `Some(true)` drives some_boolean and swap_boolean on the literal, plus
    // option_some_none on the whole `Some(true)` call.
    Some(true)
}

fn option_some_none(x: i32) -> Option<i32> {
    // `Some(x)` drives option_some_none (Some(x) -> None) only.
    Some(x)
}

fn remove_try(r: Result<i32, ()>) -> Result<i32, ()> {
    // `r?` drives remove_try (r? -> r). `Ok(v)` wraps an identifier, so
    // ok_err_boolean stays dormant.
    let v = r?;
    Ok(v)
}

fn unwrap_to_unwrap_or_default(opt: Option<i32>) -> i32 {
    // `unwrap` drives unwrap_to_unwrap_or_default; it is outside every predicate
    // operator's curated set.
    opt.unwrap()
}

fn min_max_swap(a: i32, b: i32) -> i32 {
    // `min` drives min_max_swap (min -> max).
    a.min(b)
}

fn match_wildcard_to_panic(n: i32) -> i32 {
    // The wildcard arm value drives match_wildcard_to_panic. The 7/100/200
    // literals avoid integer_zero_one, and `7` is not a boolean pattern.
    match n {
        7 => 100,
        _ => 200,
    }
}

fn empty_vec_macro() -> Vec<i32> {
    // `vec![3, 4, 5]` drives empty_vec_macro. The 3/4/5 literals avoid
    // integer_zero_one.
    vec![3, 4, 5]
}

fn saturating_checked_swap(a: u32, b: u32) -> Option<u32> {
    // `checked_add` drives saturating_checked_swap (checked_add -> saturating_add).
    a.checked_add(b)
}

fn expect_to_unwrap_or_default(opt: Option<i32>) -> i32 {
    // The whole `opt.expect(..)` call drives expect_to_unwrap_or_default; the
    // message argument is dropped, not left behind.
    opt.expect("must be present")
}
