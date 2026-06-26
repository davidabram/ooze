; Remove the `?` from a try expression: `foo()?` -> `foo()`. The whole
; try_expression is the @target; the replacement strips the trailing `?`.
(try_expression) @target
