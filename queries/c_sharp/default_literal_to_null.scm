; Default literal to null: `default` -> `null`. The grammar uses one node for
; both the bare literal and `default(T)`; the replacement only fires on the
; bare literal, so `default(T)` never mutates. Switch `default:` labels are a
; different token and are never matched.
(default_expression) @target
