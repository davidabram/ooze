; Swap a `min`/`max` call for its opposite. Two patterns cover method calls
; (`a.min(b)`) and free-function calls (`min(a, b)`); both capture the name as
; @target and the replacement swaps the curated pair.
(call_expression
  function: (field_expression
    field: (field_identifier) @target)
  (#any-of? @target "min" "max"))

(call_expression
  function: (identifier) @target
  (#any-of? @target "min" "max"))
