function plain()
    return 1
end

function if_else(x)
    if x > 0 then
        return 1
    elseif x < 0 then
        return 2
    else
        return 3
    end
end

function loops(n)
    local s = 0
    for i = 0, n - 1 do
        s = s + i
    end
    while s > 10 do
        s = s - 1
    end
    repeat
        s = s - 1
    until s <= 0
    return s
end

function bool_ops(a, b, c)
    return a and b or c
end

function try_catch(x)
    local ok, err = pcall(function()
        if x then
            error("fail")
        end
        return 2
    end)
    if ok then
        return err
    else
        return 3
    end
end

function with_closure(x)
    local add = function(a, b)
        if a > b then
            return a
        end
        return b
    end
    return add(x, x)
end
