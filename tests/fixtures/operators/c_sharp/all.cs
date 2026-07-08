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

    public static string NullForgiving(string? value)
    {
        return value!;
    }

    public static string? NullableAccess(User? user)
    {
        return user?.Name;
    }

    public static string? NullableCall(User? user)
    {
        return user?.GetName();
    }

    public static bool IsString(object value)
    {
        return value is string;
    }

    public static bool IsNotString(object value)
    {
        return value is not string;
    }

    public static bool IsNull(object? value)
    {
        return value is null;
    }

    public static bool IsPositivePattern(int value)
    {
        return value is > 0;
    }

    public static string? AsExpression(object value)
    {
        return value as string;
    }

    public static int CheckedAdd(int a, int b)
    {
        return checked(a + b);
    }

    public static int UncheckedAdd(int a, int b)
    {
        return unchecked(a + b);
    }

    public static string ThrowExpression(string? value)
    {
        return value ?? throw new ArgumentNullException(nameof(value));
    }

    public static string? DefaultLiteral()
    {
        return default;
    }
}

public sealed class User
{
    public string Name { get; set; } = "";
    public string GetName() => Name;
}
