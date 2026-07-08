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

    public static string Ignore()
    {
        // true == false
        return "x == y && true";
    }
}
