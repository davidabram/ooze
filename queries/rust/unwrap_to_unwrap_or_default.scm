; `x.unwrap()` -> `x.unwrap_or_default()`. `unwrap()` takes no arguments, so
; swapping just the method name is safe. The `#eq?` predicate scopes the match.
(call_expression
  function: (field_expression
    field: (field_identifier) @target)
  (#eq? @target "unwrap"))
