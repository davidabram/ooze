; Safe cast to direct cast: `value as T` -> `(T)value`. The whole as
; expression is the @target; the replacement splits on the top-level `as`
; keyword and rebuilds a cast expression, parenthesizing compound operands.
(as_expression) @target
