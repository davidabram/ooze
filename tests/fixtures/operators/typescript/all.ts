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

// Advanced operators below are all default_enabled = false; raw `mutants`
// discovery enables every operator, so they still appear in expected.json.

export function fallback(value: string | null, fallbackValue: string): string {
  // `value ?? fallbackValue` drives nullish_coalescing_removal (-> value).
  return value ?? fallbackValue;
}

export function optionalAccess(user: { name: string } | null): string | undefined {
  // `user?.name` drives optional_chaining_removal (-> user.name).
  return user?.name;
}

export function optionalCall(fn: (() => number) | null): number | undefined {
  // `fn?.()` drives optional_chaining_removal (-> fn()).
  return fn?.();
}

export function choose(flag: boolean, a: number, b: number): number {
  // `flag ? a : b` drives ternary_arm_swap (-> flag ? b : a).
  return flag ? a : b;
}

export function arrayLiteral(): number[] {
  // `[1, 2, 3]` drives array_empty_literal (-> []).
  return [1, 2, 3];
}

export function objectLiteral(): { a: number; b: number } {
  // `{ a: 1, b: 2 }` drives object_empty_literal (-> {}).
  return { a: 1, b: 2 };
}

export function stringLiteral(): string {
  // `"hello"` drives string_empty_literal (-> "").
  return "hello";
}

export async function awaitValue(promise: Promise<number>): Promise<number> {
  // `await promise` drives await_removal (-> promise).
  return await promise;
}
