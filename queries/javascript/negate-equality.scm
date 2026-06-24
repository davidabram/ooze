; Both loose and strict equality are swapped to their negation and back
; (== <-> !=, === <-> !==).
(binary_expression
  operator: [
    "=="
    "!="
    "==="
    "!=="
  ] @target)
