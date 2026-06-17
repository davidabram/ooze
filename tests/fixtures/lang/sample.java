package com.example;

public class Sample {
    int plain() {
        return 1;
    }

    int ifElse(int x) {
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
        for (int v : new int[] { 1, 2, 3 }) {
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

    int switchCase(int x) {
        switch (x) {
            case 1:
                return 1;
            case 2:
                return 2;
            default:
                return 3;
        }
    }

    int switchExpr(int x) {
        return switch (x) {
            case 1 -> 1;
            case 2 -> 2;
            default -> 3;
        };
    }

    int ternary(int x) {
        return x > 0 ? 1 : 2;
    }

    boolean boolOps(boolean a, boolean b, boolean c) {
        return a && b || c;
    }

    int tryCatch(boolean x) {
        try {
            if (x) {
                return 1;
            }
            return 2;
        } catch (Exception e) {
            return 3;
        }
    }

    int withLambda(int x) {
        java.util.function.Function<Integer, Integer> f = a -> {
            if (a > 0) {
                return a;
            }
            return 0;
        };
        return f.apply(x);
    }
}
