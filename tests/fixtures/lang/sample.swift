func plain() -> Int {
    return 1
}

func ifElse(_ x: Int) -> Int {
    if x > 0 {
        return 1
    } else if x < 0 {
        return 2
    } else {
        return 3
    }
}

func loops(_ n: Int) -> Int {
    var s = 0
    for i in 0..<n {
        s += i
    }
    while s > 10 {
        s -= 1
    }
    repeat {
        s -= 1
    } while s > 0
    return s
}

func switchCase(_ x: Int) -> Int {
    switch x {
    case 1:
        return 1
    case 2:
        return 2
    default:
        return 3
    }
}

func ternary(_ x: Int) -> Int {
    return x > 0 ? 1 : 2
}

func boolOps(_ a: Bool, _ b: Bool, _ c: Bool) -> Bool {
    return a && b || c
}

func tryCatch(_ x: Bool) -> Int {
    do {
        if x {
            return 1
        }
        throw NSError(domain: "", code: 1)
    } catch {
        return 3
    }
}

func nullCoalesce(_ a: Int?, _ b: Int) -> Int {
    return a ?? b
}

func withClosure(_ x: Int) -> Int {
    let add = { (a: Int, b: Int) -> Int in
        if a > b {
            return a
        }
        return b
    }
    return add(x, x)
}

func guardDemo(_ x: Int?) -> Int {
    guard let val = x else {
        return 0
    }
    return val
}
