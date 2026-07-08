; Regular string literals only: `"hello"` -> `""`. Verbatim (@"..."), raw
; ("""..."""), and interpolated ($"...") strings are distinct node kinds and
; deliberately not matched.
(string_literal) @target
