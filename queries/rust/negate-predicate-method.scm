; Negate a boolean-returning predicate method call by wrapping it in `!`.
; The whole call expression is the @target; the replacement prepends `!`.
; Only a curated set of predicate methods (all returning `bool`) is matched, so
; the negation is always type-correct. `is_some`/`is_none`/`is_ok`/`is_err` are
; intentionally left to `swap_predicate_method`, which has a more precise swap.
(call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  (#any-of? @_method
    "is_empty"
    "contains"
    "starts_with"
    "ends_with")) @target
