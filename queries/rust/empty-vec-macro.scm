; Empty a `vec!` literal: `vec![a, b, c]` -> `vec![]`. The whole macro
; invocation is the @target; the replacement rewrites it textually.
(macro_invocation
  macro: (identifier) @_macro
  (#eq? @_macro "vec")) @target
