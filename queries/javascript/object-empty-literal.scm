; Object literal emptying: `{ a: 1 }` -> `{}`. Matching the `object` node (not a
; `statement_block`) plus at least one property skips already-empty `{}`
; literals; the @target is the whole object and the replacement returns `{}`.
(object
  (_) @property) @target
