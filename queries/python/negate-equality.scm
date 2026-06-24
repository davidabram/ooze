; Python folds equality into `comparison_operator`; the `==`/`!=` tokens are
; anonymous children, matched directly under that node.
(comparison_operator
  [
    "=="
    "!="
  ] @target)
