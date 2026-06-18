[
  (conditional)
  (guard)
  (alternative)
  (multi_way_if)
  (list_comprehension)
] @branch

(infix
  operator: (operator) @_op
  (#match? @_op "^(&&|\\|\\|)$")) @branch
