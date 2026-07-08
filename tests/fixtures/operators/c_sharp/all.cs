public static class Sample
{
    public static bool IsReady(bool enabled)
    {
        if (enabled == true)
        {
            return true;
        }

        return false;
    }

    public static int Clamp(int x)
    {
        if (x < 0)
        {
            return 0;
        }

        if (x > 1)
        {
            return 1;
        }

        return x;
    }

    public static bool Both(bool a, bool b)
    {
        return a && b;
    }

    public static int Arithmetic(int a, int b)
    {
        return a + b - a * b / 2 % 3;
    }

    public static int Assignments(int x)
    {
        x += 1;
        x -= 1;
        x *= 2;
        x /= 2;
        return x;
    }

    public static int Negate(int x)
    {
        return -x;
    }

    public static int Reaffirm(int x)
    {
        return +x;
    }

    public static bool Not(bool enabled)
    {
        return !enabled;
    }

    public static string EmptyString()
    {
        return "hello";
    }

    public static bool IsMissing(string? value)
    {
        return value == null;
    }

    public static bool IsPresent(string? value)
    {
        return value != null;
    }

    public static string WithFallback(string? value)
    {
        return value ?? "fallback";
    }

    public static int Ternary(bool enabled)
    {
        return enabled ? 1 : 0;
    }

    public static int TernaryNegated(bool enabled)
    {
        return !enabled ? 1 : 0;
    }

    public static string Ignore()
    {
        // true == false
        return "x == y && true";
    }
}
