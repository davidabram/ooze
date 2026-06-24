; Boolean literal in explicit return position: `return true` <-> `return false`.
; Distinct from `swap_boolean` so return-value mutations get their own operator
; stats and test suggestions.
(return_expression
  (boolean_literal) @target)
