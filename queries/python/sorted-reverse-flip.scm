; A `sorted(...)` call. The whole call expression is the @target; the
; replacement adds or toggles a `reverse=...` keyword argument to flip the
; ordering. Default-disabled at the operator level.
(call
  function: (identifier) @_fn
  (#eq? @_fn "sorted")) @target
