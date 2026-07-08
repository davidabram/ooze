; Is-pattern negation: `x is P` <-> `x is not P`, covering type checks
; (`x is string`), constant patterns (`x is null`), and relational patterns
; (`x is > 0`). Both node kinds are matched because the grammar parses
; `x is T` as either `is_expression` (plain type) or `is_pattern_expression`
; (any pattern). The whole expression is the @target; the replacement toggles
; `not` after the top-level `is`. `x == null` is a binary_expression and is
; negate_equality's job, never matched here.
[
  (is_expression)
  (is_pattern_expression)
] @target
