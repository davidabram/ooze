(call
  target: (identifier) @_target
  (arguments . (identifier) @fn.name)
  (do_block)
  (#match? @_target "^(def|defp)$")) @fn.def

(anonymous_function) @fn.def
