; Boolean literal in explicit return position: `return True` <-> `return False`.
; Python spells the literals as the named `true`/`false` nodes (mirroring
; swap-boolean.scm). Distinct from `swap_boolean` so return-value mutations get
; their own operator stats and test suggestions.
(return_statement
  (true) @target)

(return_statement
  (false) @target)
