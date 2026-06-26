; Iterator predicate quantifier: `xs.some(fn)` <-> `xs.every(fn)`. The
; `#any-of?` predicate restricts matches to this curated pair and the
; replacement swaps the captured method name. Both take a callback and return a
; boolean, so the swap stays well-typed.
(call_expression
  function: (member_expression
    property: (property_identifier) @target)
  arguments: (arguments (_))
  (#any-of? @target "some" "every"))
