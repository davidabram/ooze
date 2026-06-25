; Iterator predicate quantifier: `xs.iter().any(..)` <-> `xs.iter().all(..)`.
; The `#any-of?` predicate restricts matches to this curated pair, and the
; replacement swaps the captured method name. Both methods take a closure and
; return `bool`, so the swap is always type-correct.
(call_expression
  function: (field_expression
    field: (field_identifier) @target)
  (#any-of? @target "any" "all"))
