; `x.expect("msg")` -> `x.unwrap_or_default()`. The whole call expression is the
; @target because the replacement must drop the message argument, not just rename
; the method (a name-only swap would leave `unwrap_or_default("msg")`). The
; replacement splits the receiver off at `.expect(`.
(call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  arguments: (arguments (_))
  (#eq? @_method "expect")) @target
