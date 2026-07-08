; Boolean literal in explicit return position: `return true` <-> `return false`.
; Overlaps swap_boolean by design; dedupe keeps this more specific operator
; (see OperatorName::dedup_priority).
(return_statement
  (boolean_literal) @target)
