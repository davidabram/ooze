// Mutation-operator discovery fixture for JavaScript.
//
// Mirrors the Rust fixture's intent: each function isolates an obvious mutation
// target so discovery can be asserted by compact shape (function, operator,
// original, replacement) without caring about line/column. Where two operators
// legitimately fire on the same node, the overlap is called out in a comment.

function swapBoolean() {
  // `true` is a plain boolean literal -> swap_boolean only.
  const enabled = true;
  return enabled;
}

function negateEquality(a, b) {
  return a == b;
}

function compare(a, b) {
  // The `<` operator drives both comparison_boundary (< -> <=) and
  // comparison_negation (< -> >=).
  return a < b;
}

function swapLogical(x, y) {
  return x && y;
}

function removeNot(flag) {
  return !flag;
}
