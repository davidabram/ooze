; Overflow-context swap: `checked(expr)` <-> `unchecked(expr)` and the block
; statement forms `checked { ... }` <-> `unchecked { ... }`. Only the keyword
; token is the @target, so the diff stays a one-word edit.
(checked_expression
  ["checked" "unchecked"] @target)
(checked_statement
  ["checked" "unchecked"] @target)
