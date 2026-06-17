function plain() {
    return 1;
}

function ifElse(x) {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return 2;
    } else {
        return 3;
    }
}

function loops(n) {
    let s = 0;
    for (let i = 0; i < n; i++) {
        s += i;
    }
    while (s > 10) {
        s--;
    }
    do {
        s--;
    } while (s > 0);
    return s;
}

function switchCase(x) {
    switch (x) {
        case 1:
            return 1;
        case 2:
            return 2;
        default:
            return 3;
    }
}

function ternary(x) {
    return x > 0 ? 1 : 2;
}

function boolOps(a, b, c) {
    return a && b || c;
}

function nullCoalesce(a, b) {
    return a ?? b;
}

function tryCatch(x) {
    try {
        if (x) {
            return 1;
        }
        return 2;
    } catch (e) {
        return 3;
    }
}

const arrow = (x) => {
    if (x) {
        return 1;
    }
    return 2;
};
