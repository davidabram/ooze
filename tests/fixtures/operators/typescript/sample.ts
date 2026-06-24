// Mutation-operator discovery fixture for TypeScript.
//
// Mirrors the JavaScript fixture (the expression grammar is the same) with light
// type annotations. Each function isolates an obvious mutation target so
// discovery can be asserted by compact shape (function, operator, original,
// replacement) without caring about line/column. Where two operators legitimately
// fire on the same node, the overlap is called out in a comment.

function swapBoolean(): boolean {
  // `true` is a plain boolean literal -> swap_boolean only.
  const enabled = true;
  return enabled;
}

function negateEquality(a: number, b: number): boolean {
  return a == b;
}

function compare(a: number, b: number): boolean {
  // The `<` operator drives both comparison_boundary (< -> <=) and
  // comparison_negation (< -> >=).
  return a < b;
}

function swapLogical(x: boolean, y: boolean): boolean {
  return x && y;
}

function removeNot(flag: boolean): boolean {
  return !flag;
}
