; TypeScript (like JavaScript) has one `number` node for all numeric literals;
; the mutator's replace fn only rewrites exact `0`/`1`, so other numbers never
; match.
(number) @target
