; Boolean wrapped in `Some`: `Some(true)` <-> `Some(false)`. More precise than
; the general `swap_boolean`, so the report hint can mention `Option<bool>`.
(call_expression
  function: (identifier) @_ctor
  arguments: (arguments
    (boolean_literal) @target)
  (#eq? @_ctor "Some"))
