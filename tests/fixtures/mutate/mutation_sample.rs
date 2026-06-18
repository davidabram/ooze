fn is_ready() -> bool {
    true
}

fn is_disabled() -> bool {
    false
}

fn check(x: i32, y: i32, enabled: bool) -> bool {
    if enabled && x == y {
        true
    } else if x >= 1 || y < 0 {
        false
    } else {
        x != y
    }
}
