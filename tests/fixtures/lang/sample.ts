function plain(): number {
    return 1;
}

function ifElse(x: number): number {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return 2;
    } else {
        return 3;
    }
}

function loops(n: number): number {
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

function switchCase(x: number): number {
    switch (x) {
        case 1:
            return 1;
        case 2:
            return 2;
        default:
            return 3;
    }
}

function ternary(x: number): number {
    return x > 0 ? 1 : 2;
}

function boolOps(a: boolean, b: boolean, c: boolean): boolean {
    return a && b || c;
}

function nullCoalesce(a: number | null, b: number): number {
    return a ?? b;
}

function tryCatch(x: boolean): number {
    try {
        if (x) {
            return 1;
        }
        return 2;
    } catch (e) {
        return 3;
    }
}

const arrow = (x: number): number => {
    if (x) {
        return 1;
    }
    return 2;
};
