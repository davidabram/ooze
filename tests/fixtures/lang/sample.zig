fn plain() i32 {
    return 1;
}

fn ifElse(x: i32) i32 {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return 2;
    } else {
        return 3;
    }
}

fn loops(n: i32) i32 {
    var s: i32 = 0;
    for (0..@intCast(n)) |i| {
        s += @as(i32, @intCast(i));
    }
    while (s > 10) {
        s -= 1;
    }
    return s;
}

fn switchCase(x: i32) i32 {
    return switch (x) {
        1 => 1,
        2 => 2,
        else => 3,
    };
}

fn boolOps(a: bool, b: bool, c: bool) bool {
    return a and b or c;
}

fn ternary(x: i32) i32 {
    return if (x > 0) 1 else 2;
}

fn tryCatch(x: bool) i32 {
    return if (x) 1 else errorElse();
}

fn errorElse() i32 {
    return 2;
}
