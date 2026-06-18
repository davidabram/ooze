[
  (if_expression)
  (while_expression)
  (do_while_expression)
  (for_expression)
  (case_clause)
  (catch_clause)
  (finally_clause)
  (try_expression)
] @branch

(infix_expression
  operator: (operator_identifier) @_op) @branch
(#match? @_op "^(&&|\\|\\|)$")
