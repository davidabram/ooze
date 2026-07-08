; Binary arithmetic operators only; unary +x / -x are prefix_unary_expression
; nodes and are handled by remove_unary_minus / plus_to_minus.
(binary_expression
  operator: [
    "+"
    "-"
    "*"
    "/"
    "%"
  ] @target)
