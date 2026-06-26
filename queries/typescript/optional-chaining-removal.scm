; Optional chaining removal: `a?.b` -> `a.b`, `fn?.()` -> `fn()`. Member and
; subscript chains carry a named `optional_chain` (`?.`) node, while an optional
; *call* in the TypeScript grammar carries an anonymous `?.` token; both forms
; are matched and the replacement rewrites the chain to its non-optional form.
[
  (member_expression (optional_chain)) @target
  (subscript_expression (optional_chain)) @target
  (call_expression "?.") @target
]
