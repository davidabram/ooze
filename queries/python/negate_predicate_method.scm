; Negate a boolean-returning string predicate call by wrapping it in `not (...)`.
; The whole call expression is the @target; the replacement wraps (or unwraps) a
; leading `not`. Only a curated set of `str` predicate methods (all returning
; `bool`) is matched, so the negation is always well-typed.
(call
  function: (attribute
    attribute: (identifier) @_method)
  arguments: (argument_list)
  (#any-of? @_method
    "isdigit"
    "isdecimal"
    "isnumeric"
    "isalpha"
    "isalnum"
    "islower"
    "isupper"
    "isspace")) @target
