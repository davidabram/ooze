; Swap saturating/checked arithmetic: `checked_add` <-> `saturating_add`,
; `checked_sub` <-> `saturating_sub`. Both forms share the same signature, so
; swapping the method name is type-correct. The `#any-of?` predicate scopes it.
(call_expression
  function: (field_expression
    field: (field_identifier) @target)
  (#any-of? @target
    "checked_add"
    "checked_sub"
    "saturating_add"
    "saturating_sub"))
