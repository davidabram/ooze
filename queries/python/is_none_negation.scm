; `x is None` / `x is not None`. Restricted to comparisons that have a `None`
; operand so this stays a nullability check rather than a generic identity flip.
; The `is` / `is not` operator token is captured and swapped in place.
(comparison_operator
  [
    "is"
    "is not"
  ] @target
  (none))
