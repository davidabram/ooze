; The condition of an `if` / `while`. Wrapping it in `not (...)` (or unwrapping an
; existing negation) flips the branch a truthiness check takes.
(if_statement
  condition: (_) @target)

(while_statement
  condition: (_) @target)
