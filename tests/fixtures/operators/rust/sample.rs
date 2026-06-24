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
