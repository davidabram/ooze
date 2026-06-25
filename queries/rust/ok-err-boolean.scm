; Boolean wrapped in a Result constructor: `Ok(true)`/`Ok(false)`/`Err(true)`/
; `Err(false)`, flipping the inner literal. The `#any-of?` predicate scopes the
; match to Ok/Err so the report hint can talk about `Result<bool, E>`.
(call_expression
  function: (identifier) @_ctor
  arguments: (arguments
    (boolean_literal) @target)
  (#any-of? @_ctor "Ok" "Err"))
