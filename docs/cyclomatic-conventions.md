# Cyclomatic Complexity Conventions

This document records the semantic decisions made when designing tree-sitter
queries for ooze's cyclomatic complexity scanner.  Cyclomatic complexity is
defined as **1 + number of decision points** in a function.

## What Counts as a Branch

Each of these constructs counts as **one** decision point (branch):

| Category | Examples |
|---|---|
| Conditionals | `if`, `elif` / `else if` / `elsif`, `unless`, `cond`, `guard` |
| Loops | `for`, `while`, `until`, `loop`, `repeat`, `do-while` |
| Pattern matching | `case` / `match` arms (each `case_clause` / `match_arm` / `when` / `stab_clause`), `switch` arms |
| Exception handling | `catch`, `rescue`, `except`, `catch_clause`, `catch_block`, `rescue_block` |
| Short-circuit booleans | `&&`, `||`, `and`, `or`, `andalso`, `orelse` |
| Ternary / conditional expr | `a ? b : c`, `if-then-else` as expression |
| List comprehension guards | `if` / `guard` qualifiers |
| Preprocessor conditionals | `#if`, `#ifdef`, `#elif` (C/C++) |
| Guard clauses | pattern guards, `when` clauses, `let_chain` |
| Assertions | `assert`, `let_assert` (Gleam) |
| `try` expression | `try_expression` (Rust, Scala) |
| `??` (null coalescing) | C#, JavaScript, TypeScript, PHP, Dart, Swift |

## What Does NOT Count

| Construct | Rationale |
|---|---|
| `else` / `else_block` / `else_statement` | `else` is the alternative path of an existing decision, not a new decision |
| `default` (switch default) | `default` is just another switch arm; already counted as a `case_clause` / `switch_case` |
| `panic` / `todo` / `throw` | These are side-effect or abort expressions, not control-flow decisions |
| `finally` / `ensure` | Always-executed cleanup, not a conditional path |
| Variable `let` bindings | Not function definitions (OCaml: `let i = ref 0` is not a function; only `let f x = ...` counts) |

## Function Detection Rules

- A function must have at least one parameter or be explicitly declared as
  `function`, `fn`, `def`, `fun`, `lambda`, etc.
- OCaml: `let_binding` with a `(parameter)` child = function; plain `let x = e`
  = variable (ignored).
- Elixir: only `def` / `defp` targets count; `if`, `case`, `cond`, `for`, `try`
  etc. are calls, not functions.
- Anonymous functions (closures, lambdas, anonymous classes) are captured with
  synthetic names like `<anonymous>:N` where N is the start line.
- Nested functions have their own cyclomatic complexity; branches inside a
  nested function are NOT charged to the enclosing function.

## Predicate Placement

**Always place predicates (`#match?`, `#eq?`) INSIDE the node they apply to**
as a sibling child, NOT on their own line after the pattern:

```scheme
;; GOOD — predicate is a child of `call`
(call
  target: (identifier) @_target
  (arguments . (identifier) @fn.name)
  (do_block)
  (#match? @_target "^(def|defp)$")) @fn.def

;; BAD — predicate is a separate pattern (unfiltered in tree-sitter 0.25)
(call
  target: (identifier) @_target
  (arguments . (identifier) @fn.name)
  (do_block)) @fn.def
(#match? @_target "^(def|defp)$")
```

In tree-sitter 0.25.10+, a predicate on its own line becomes a distinct pattern
with no capture steps. The capture pattern then has empty `text_predicates` and
all matches pass unfiltered.

## Node Type Matching Rules

Leaf/unnamed tokens:

```scheme
(binary_expression operator: ["&&" "||"])  ;; operator is an anonymous token
```

Named nodes:

```scheme
(infix_expression operator: (and_operator))  ;; OCaml — operator is a named node
(infix_expression operator: (or_operator))
```

When a grammar `alias()`es an operator to a named node (e.g. Julia's
`alias($._lazy_and_operator, $.operator)`), you cannot match the underlying
operator text without a predicate.  Prefer literal token matching when
available; fall back to inline `#match?` predicates only when the grammar
provides no anonymous token for the operator.

## Per-Language Notes

### OCaml
- Boolean `&&`/`||` operators are named nodes `and_operator` / `or_operator`,
  NOT string literals.
- `fun_expression` (`fun x -> ...`) is a separate node from
  `function_expression` (`function ... -> ...`); both are anonymous functions.

### Elixir
- `if`, `case`, `cond`, `for`, `try` etc. are all `call` nodes, not language
  keywords.  Functions are only `def`/`defp` calls.
- `else` in an `if`/`else` is an `else_block` — NOT counted.
- Stab clauses (`->` in `case`/`cond`) are the branch units.

### Erlang
- `andalso` / `orelse` are anonymous tokens inside `binary_op_expr` (no
  `operator:` field).  Match them as bare string children:
  `(binary_op_expr "andalso") @branch`.

### Gleam
- `case` is the container; `case_clause` is each branch.  Only `case_clause`
  counts.
- `panic` / `todo` are abort expressions, NOT branches.
- `assert` / `let_assert` are decision points (may fail).

### Haskell
- `list_comprehension` qualifies as a branch.
- Boolean operators `&&`/`||` use inline `#match?` predicate on
  `(infix operator: (operator) @_op)`.

### Julia
- Function signatures are wrapped in a `signature` node (the grammar's
  `choice()` produces a named wrapper).
- Boolean operators `&&`/`||` are aliased to the named node `(operator)` via
  `alias($._lazy_and_operator, $.operator)`, making text-level matching
  impossible without predicates.

### Lua
- `else_statement` is NOT a branch.
- `function_definition` (no name) is the anonymous function expression;
  `function_declaration` (with name) is the named form.

### Scala
- Boolean operators `&&`/`||` match via inline `#match?` on
  `(infix_expression operator: (operator_identifier) @_op)`.
- Without the predicate, ALL infix operators (`+`, `>`, `+=`, etc.) are
  incorrectly counted.

### Zig
- `switch_expression` is the container; `switch_case` are the branches.  Only
  `switch_case` counts.
- `if_expression` / `if_statement` are both valid (expression vs statement
  context); both count.
