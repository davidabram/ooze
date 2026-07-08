; Ternary arm swap: `cond ? a : b` -> `cond ? b : a`. The whole conditional
; expression is the @target; the replacement splits only the top-level `?` and
; its matching top-level `:` and swaps the two result arms. `if` statements are
; a different node and are never matched.
(conditional_expression) @target
