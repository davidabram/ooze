; Array literal emptying: `[a, b]` -> `[]`. The `(_) @item` ensures at least one
; element so already-empty `[]` literals are skipped; the @target is the whole
; array and the replacement returns `[]`.
(array
  (_) @item) @target
