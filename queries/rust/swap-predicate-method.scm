; Method-call receivers like `opt.is_some()`. The replacement function only
; swaps a curated set of opposite predicate pairs (is_some/is_none,
; is_ok/is_err) and skips every other method name.
(call_expression
  function: (field_expression
    field: (field_identifier) @target))
