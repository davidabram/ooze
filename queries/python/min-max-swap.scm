; Swap a built-in `min`/`max` call for its opposite. The `#any-of?` predicate
; restricts matches to this curated pair, capturing the name as @target; the
; replacement swaps it. Default-disabled at the operator level.
(call
  function: (identifier) @target
  arguments: [
    (argument_list (_))
    (generator_expression)
  ]
  (#any-of? @target "min" "max"))
