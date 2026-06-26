; A method call `recv.method(args)`. The whole call is captured; the replacement
; confirms the method is `get` and that it carries exactly two arguments before
; dropping the default.
(call
  function: (attribute
    attribute: (identifier))
  arguments: (argument_list)) @target
