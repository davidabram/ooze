; Throw expression to null: `x ?? throw new ArgumentNullException(...)` ->
; `x ?? null`. Only expression-position throws are matched; `throw` statements
; are a different node (throw_statement) and are never touched.
(throw_expression) @target
