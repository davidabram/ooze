; String emptying: `"hello"` -> `""`, `'hello'` -> `''`, `` `hello` `` -> ``` `` ```.
; The replacement skips already-empty strings and template strings that contain
; `${...}` interpolation.
[
  (string) @target
  (template_string) @target
]
