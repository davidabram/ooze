; Only ranges with an end bound: `..` <-> `..=` is a no-op of meaning but a
; valid swap only when there is something to include/exclude. The trailing
; anchor keeps the operator's end expression as the last child, so endless
; (`0..`) and full (`..`) ranges — where `..=` would be invalid syntax — are
; left alone.
(range_expression
  [
    ".."
    "..="
  ] @target
  (_) .)
