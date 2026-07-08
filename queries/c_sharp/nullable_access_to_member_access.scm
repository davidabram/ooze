; Null-propagating access removal: `user?.Name` -> `user.Name`,
; `items?[0]` -> `items[0]`. The whole conditional access expression is the
; @target; the replacement drops each `?` that starts a `?.` or `?[` binding
; (quote-aware, so `?.` inside a string argument is never touched). Nested
; chains produce one candidate per conditional access node.
(conditional_access_expression) @target
