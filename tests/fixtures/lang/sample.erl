-module(sample).
-export([plain/0, if_else/1, case_demo/1, loops/1, bool_ops/3, try_catch/1, list_comp/1]).

plain() ->
    1.

if_else(X) ->
    if
        X > 0 -> 1;
        X < 0 -> 2;
        true   -> 3
    end.

case_demo(X) ->
    case X of
        1 -> 1;
        2 -> 2;
        _ -> 3
    end.

loops(N) ->
    S = lists:foldl(fun(I, Acc) -> Acc + I end, 0, lists:seq(0, N - 1)),
    if
        S > 10 -> S - 1;
        true   -> S
    end.

bool_ops(A, B, C) ->
    A andalso B orelse C.

try_catch(X) ->
    try
        case X of
            true  -> 1;
            false -> throw(error)
        end
    catch
        _:_ -> 3
    end.

list_comp(Xs) ->
    [X || X <- Xs, X > 0].
