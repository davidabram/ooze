; A `recv.get(...)` method call. The whole call expression is the @target; the
; replacement rewrites a single-argument `recv.get(key)` to `recv.get[key]`-style
; subscription (`recv[key]`). Two-argument `.get(key, default)` is left to
; dict_get_default_removal. Default-disabled at the operator level.
(call
  function: (attribute
    attribute: (identifier) @_method)
  arguments: (argument_list)
  (#eq? @_method "get")) @target
