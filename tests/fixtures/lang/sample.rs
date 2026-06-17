fn plain() -> u32 {
    1
}

fn if_else(x: bool) -> u32 {
    if x {
        1
    } else {
        2
    }
}

fn if_elif_else(x: i32) -> u32 {
    if x > 0 {
        1
    } else if x < 0 {
        2
    } else {
        3
    }
}

fn loops_and_match(xs: &[u32]) -> u32 {
    let mut sum = 0;
    for &x in xs {
        sum += x;
    }
    let mut i = 0;
    while i < 3 {
        sum += i;
        i += 1;
    }
    match sum {
        0 => 0,
        1 => 1,
        _ => sum,
    }
}

fn bool_ops(a: bool, b: bool, c: bool) -> bool {
    a && b || c
}

fn try_op(x: Option<u32>) -> Option<u32> {
    Some(x? + 1)
}

fn let_chain_opt(a: Option<u32>, b: Option<u32>) -> u32 {
    if let Some(x) = a && let Some(y) = b {
        x + y
    } else {
        0
    }
}

fn outer_with_closure() -> u32 {
    let add = |a: u32, b: u32| {
        if a > b {
            a
        } else {
            b
        }
    };
    add(1, 2)
}
