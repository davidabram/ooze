; Postfix null-forgiving removal: `value!` -> `value`. The anchored `!` token
; distinguishes the null-forgiving suffix from `x++`/`x--` (same node, other
; operators) and from prefix logical not (`prefix_unary_expression`, handled
; by remove_not). The whole postfix expression is the @target; the replacement
; strips the trailing `!`.
(postfix_unary_expression
  "!") @target
