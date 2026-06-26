; `Some(value) -> None`. The whole call expression is the @target so the
; replacement rewrites it textually; matching is scoped to the `Some`
; constructor by the `#eq?` predicate.
(call_expression
  function: (identifier) @_ctor
  arguments: (arguments (_))
  (#eq? @_ctor "Some")) @target
