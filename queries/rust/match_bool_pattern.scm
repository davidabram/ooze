; Boolean literal in a match-arm pattern: `true => ...` <-> `false => ...`.
; Distinct from `swap_boolean` (which also matches the literal) so match-based
; boolean branching gets its own operator stats and test suggestions.
(match_arm
  pattern: (match_pattern
    (boolean_literal) @target))
