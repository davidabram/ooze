int Plain()
{
    return 1;
}

int IfElse(int x)
{
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return 2;
    } else {
        return 3;
    }
}

int Loops(int n)
{
    int s = 0;
    for (int i = 0; i < n; i++) {
        s += i;
    }
    foreach (int v in new[] { 1, 2, 3 }) {
        s += v;
    }
    while (s > 10) {
        s--;
    }
    do {
        s--;
    } while (s > 0);
    return s;
}

int SwitchCase(int x)
{
    switch (x) {
        case 1:
            return 1;
        case 2:
            return 2;
        default:
            return 3;
    }
}

int SwitchExpr(int x)
{
    return x switch
    {
        1 => 1,
        2 => 2,
        _ => 3
    };
}

int Ternary(int x)
{
    return x > 0 ? 1 : 2;
}

int BoolOps(int a, int b, int c)
{
    return a && b || c;
}

int NullCoalesce(int? x)
{
    return x ?? 0;
}

int TryCatch(int x)
{
    try {
        if (x) {
            return 1;
        }
        return 2;
    } catch (System.Exception) {
        return 3;
    }
}

int CatchFilter(int x)
{
    try {
        return 1;
    } catch (System.Exception e) when (x > 0) {
        return 2;
    }
}

int CaseGuard(int x)
{
    switch (x) {
        case 1 when x > 0:
            return 1;
        default:
            return 2;
    }
}

int WithLambda(int x)
{
    System.Func<int, int, int> f = (a, b) =>
    {
        if (a > b) {
            return a;
        }
        return b;
    };
    return f(x, x);
}
