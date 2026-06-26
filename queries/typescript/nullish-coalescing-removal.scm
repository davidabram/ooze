; Nullish coalescing fallback removal: `a ?? b` -> `a`. The whole binary
; expression is the @target; the replacement splits on the top-level `??` and
; keeps the left operand, dropping the fallback.
(binary_expression
  operator: "??") @target
