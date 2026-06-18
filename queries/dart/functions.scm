(function_declaration
  signature: (function_signature
    name: (identifier) @fn.name)
  body: (_)) @fn.def

(method_declaration
  signature: (method_signature
    (function_signature
      name: (identifier) @fn.name))
  body: (_)) @fn.def

(constructor_signature
  name: (identifier) @fn.name) @fn.def

(local_function_declaration) @fn.def

(function_expression) @fn.def
