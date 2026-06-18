function plain()
    return 1
end

function if_else(x)
    if x > 0
        return 1
    elseif x < 0
        return 2
    else
        return 3
    end
end

function loops(n)
    s = 0
    for i in 0:(n - 1)
        s += i
    end
    while s > 10
        s -= 1
    end
    return s
end

function ternary(x)
    return x > 0 ? 1 : 2
end

function switch_case(x)
    if x == 1
        return 1
    elseif x == 2
        return 2
    else
        return 3
    end
end

function bool_ops(a, b, c)
    return a && b || c
end

function try_catch(x)
    try
        if x
            return 1
        end
        error("fail")
    catch e
        return 3
    end
end

function list_comp(xs)
    return [x for x in xs if x > 0]
end
