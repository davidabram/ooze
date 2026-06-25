; Replace a wildcard match arm's value with a panic: `_ => expr` becomes
; `_ => panic!("ooze mutant")`. The wildcard `_` is a node literally named `_`
; (quoted here so it isn't read as the query wildcard); the arm value is @target.
(match_arm
  pattern: (match_pattern
    ("_"))
  value: (_) @target)
