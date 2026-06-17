(function_declaration
  name: (identifier) @fn.name
  body: (block)) @fn.def

(method_declaration
  receiver: (parameter_list) @class.name
  name: (field_identifier) @fn.name
  body: (block)) @fn.def

(func_literal) @fn.def
