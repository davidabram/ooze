; Compound assignment operators only; plain `=` and `%=` are deliberately
; excluded from the initial set.
(assignment_expression
  operator: [
    "+="
    "-="
    "*="
    "/="
  ] @target)
