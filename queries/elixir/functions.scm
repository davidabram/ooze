(call
  target: (identifier) @_target
  (arguments . (identifier) @fn.name)
  (do_block)) @fn.def
(#match? @_target "^(def|defp)$")

(anonymous_function) @fn.def
