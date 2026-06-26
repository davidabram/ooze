; Ternary arm swap: `cond ? a : b` -> `cond ? b : a`. The whole ternary is the
; @target; the replacement splits only the top-level `?` and its matching
; top-level `:` and swaps the two result arms.
(ternary_expression) @target
