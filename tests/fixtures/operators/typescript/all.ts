// Snapshot fixture: one tiny example for every TypeScript mutation operator.
//
// The companion `expected.json` pins the discovered mutants by stable fields
// only (language, operator, implementation, function, original, replacement,
// line). Unstable fields — absolute paths, byte offsets, the path-qualified id,
// and any test-runner output — are intentionally not snapshotted. Mirrors the
// JavaScript fixture with light type annotations; where two operators
// legitimately fire on the same node, the overlap is noted in a comment.

export function core(a: number, b: number, flag: boolean = true): boolean {
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

export function quantifier(items: Item[]): boolean {
  // `some(...)` drives iterator_any_all (some -> every).
  return items.some(x => x.active);
}

export function boundary(name: string): boolean {
  // `startsWith`/`endsWith` drive string_boundary_method_swap; `||` drives
  // swap_logical.
  return name.startsWith("pre") || name.endsWith(".txt");
}

export function membership(items: string[], item: string): boolean {
  // `items.includes(item)` drives includes_negation (wrap in `!(...)`).
  return items.includes(item);
}
