// Snapshot fixture: one tiny example for every JavaScript mutation operator.
//
// The companion `expected.json` pins the discovered mutants by stable fields
// only (language, operator, implementation, function, original, replacement,
// line). Unstable fields — absolute paths, byte offsets, the path-qualified id,
// and any test-runner output — are intentionally not snapshotted. Keep each
// function minimal; where two operators legitimately fire on the same node, the
// overlap is noted in a comment.

export function core(a, b, flag = true) {
  // `!flag` drives remove_not; `flag && ...` drives swap_logical; `a === b`
  // drives negate_equality. The `flag = true` default drives swap_boolean; each
  // `return true`/`return false` drives return_boolean (the overlapping
  // swap_boolean mutant is deduped away); `a < b` drives both comparison operators.
  if (!flag) {
    return false;
  }

  if (flag && a === b) {
    return true;
  }

  return a < b;
}

export function quantifier(items) {
  // `some(...)` drives iterator_any_all (some -> every).
  return items.some(x => x.active);
}

export function boundary(name) {
  // `startsWith`/`endsWith` drive string_boundary_method_swap; `||` drives
  // swap_logical.
  return name.startsWith("pre") || name.endsWith(".txt");
}

export function membership(items, item) {
  // `items.includes(item)` drives includes_negation (wrap in `!(...)`).
  return items.includes(item);
}

// Advanced operators below are all default_enabled = false; raw `mutants`
// discovery enables every operator, so they still appear in expected.json.

export function fallback(value, fallbackValue) {
  // `value ?? fallbackValue` drives nullish_coalescing_removal (-> value).
  return value ?? fallbackValue;
}

export function optionalAccess(user) {
  // `user?.name` drives optional_chaining_removal (-> user.name).
  return user?.name;
}

export function optionalCall(fn) {
  // `fn?.()` drives optional_chaining_removal (-> fn()).
  return fn?.();
}

export function choose(flag, a, b) {
  // `flag ? a : b` drives ternary_arm_swap (-> flag ? b : a).
  return flag ? a : b;
}

export function arrayLiteral() {
  // `[1, 2, 3]` drives array_empty_literal (-> []).
  return [1, 2, 3];
}

export function objectLiteral() {
  // `{ a: 1, b: 2 }` drives object_empty_literal (-> {}).
  return { a: 1, b: 2 };
}

export function stringLiteral() {
  // `"hello"` drives string_empty_literal (-> "").
  return "hello";
}

export async function awaitValue(promise) {
  // `await promise` drives await_removal (-> promise).
  return await promise;
}
