; Negate an includes/membership predicate by wrapping the whole call in `!`.
; The whole call expression is the @target; the replacement wraps (or unwraps) a
; leading `!`. Only `.includes(x)` calls are matched, so the negation flips a
; boolean predicate.
(call_expression
  function: (member_expression
    property: (property_identifier) @_method)
  arguments: (arguments (_))
  (#eq? @_method "includes")) @target
