; `x in y` / `x not in y`. The membership operator token lives directly under
; `comparison_operator` (the `in` in a `for ... in` is a different node, so it is
; not matched here).
(comparison_operator
  [
    "in"
    "not in"
  ] @target)
