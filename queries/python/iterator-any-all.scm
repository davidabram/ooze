; Built-in quantifier swap: `any(items)` <-> `all(items)`. The `#any-of?`
; predicate restricts matches to this curated pair and the replacement swaps the
; captured function name. Both take one iterable and return `bool`, so the swap
; stays well-typed.
(call
  function: (identifier) @target
  arguments: [
    (argument_list (_))
    (generator_expression)
  ]
  (#any-of? @target "any" "all"))
