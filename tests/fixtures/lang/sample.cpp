int plain() {
    return 1;
}

int if_else(int x) {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return 2;
    } else {
        return 3;
    }
}

int loops(int n) {
    int s = 0;
    for (int i = 0; i < n; i++) {
        s += i;
    }
    int arr[] = {1, 2, 3};
    for (int v : arr) {
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

int switch_case(int x) {
    switch (x) {
        case 1:
            return 1;
        case 2:
            return 2;
        default:
            return 3;
    }
}

int ternary(int x) {
    return x > 0 ? 1 : 2;
}

int bool_ops(int a, int b, int c) {
    return a && b || c;
}

int try_catch(int x) {
    try {
        if (x) {
            return 1;
        }
        return 2;
    } catch (...) {
        return 3;
    }
}

int preproc_inside(int x) {
#if X > 0
    if (x) {
        return 1;
    }
    return 2;
#elif X < 0
    return 3;
#else
    return 4;
#endif
}

int with_lambda(int x) {
    auto f = [](int a, int b) {
        if (a > b) {
            return a;
        }
        return b;
    };
    return f(x, x);
}
