; Optional chaining removal: `a?.b` -> `a.b`, `fn?.()` -> `fn()`. Match the
; member/call/subscript expression that carries an `optional_chain` (`?.`) token
; and rewrite the chain to its non-optional form.
[
  (member_expression (optional_chain)) @target
  (call_expression (optional_chain)) @target
  (subscript_expression (optional_chain)) @target
]
