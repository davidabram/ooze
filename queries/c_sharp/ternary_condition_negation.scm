; Ternary condition negation: `cond ? a : b` -> `!(cond) ? a : b`. Only the
; condition of a conditional expression is the @target; the replacement wraps
; it in `!(...)`, or strips a leading `!` when the condition is already
; negated (which dedupes against `remove_not` at the same site).
(conditional_expression
  condition: (_) @target)
