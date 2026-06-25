; Swap string boundary methods: `name.startsWith(x)` <-> `name.endsWith(x)`.
; The `#any-of?` predicate restricts matches to this curated pair; the
; replacement swaps the captured method name. Both take one argument and return
; a boolean, so the swap stays well-typed.
(call_expression
  function: (member_expression
    property: (property_identifier) @target)
  arguments: (arguments (_))
  (#any-of? @target "startsWith" "endsWith"))
