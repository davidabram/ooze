; A comparison between a `len(...)`-shaped call and an integer (e.g.
; `len(x) == 0`). The whole comparison is captured; the replacement confirms the
; `len(...) <op> 0` shape from its text before rewriting the operator.
(comparison_operator
  (call
    function: (identifier))
  (integer)) @target
