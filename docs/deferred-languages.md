# Deferred languages

Languages that were considered for inclusion but dropped because their published
`tree-sitter-<lang>` crates on crates.io are pinned to an old `tree-sitter`
version and expose the legacy `pub fn language() -> Language` API, which is
incompatible with this project's `tree-sitter = "0.25"` (modern crates expose
`pub const LANGUAGE: LanguageFn` and bridge via `.into()`).

When a 0.25-compatible crate is published — or a custom in-tree grammar lib is
written — add these languages following the pattern in `src/lang/rust.rs`.

## Kotlin

- Crate: `tree-sitter-kotlin = "0.3.5"` (latest 0.3.8)
- Grammar repo: https://github.com/fwcd/tree-sitter-kotlin
- tree-sitter dep: `0.20`
- API: `pub fn language() -> Language` (legacy)
- Extensions: `kt`, `kts`
- What it is: JVM language used primarily for Android and server-side
  development; statically typed, interoperable with Java.
- Function node types (from `src/node-types.json`): `function_declaration`,
  `function_body`, `getter`, `setter`, `anonymous_function` (lambdas).
- Branch node types: `if_expression`, `for_expression`, `while_expression`,
  `do_while_expression`, `when_expression` / `when_entry`, `try_expression`,
  `catch_block`, `finally_block`, `conditional_expression` (`a ? b : c`),
  `binary_expression` with `&&` / `||` / `?:` (elvis).

## SQL

- Crate: `tree-sitter-sql = "0.0.2"`
- Grammar repo: https://github.com/m-novikov/tree-sitter-sql
- tree-sitter dep: `0.19.3`
- API: `pub fn language() -> Language` (legacy)
- Extensions: `sql`
- What it is: structured query language for relational databases; note that
  cyclomatic complexity is unusual for SQL (no loops/exceptions), branches are
  mostly `CASE` arms and boolean `AND`/`OR` in `WHERE`/`ON` predicates.
- Function node types: SQL has no first-class functions in the procedural
  sense; treat `create_function_statement`, `create_procedure_statement`, or
  stored-procedure bodies as the "function" unit depending on dialect support.
- Branch node types: `case`, `when_clause`, `else_clause`, `if_statement`
  (procedural extensions), boolean operators `AND` / `OR` in `where_clause`.

## How to re-add

1. Add the dep to `Cargo.toml` (or vendor a custom grammar lib).
2. Create `src/lang/<lang>.rs` mirroring `src/lang/rust.rs`, returning the
   grammar's `LANGUAGE` (or `language()` for legacy) via `.into()`.
3. Create `queries/<lang>/functions.scm` and `queries/<lang>/branches.scm`
   using the node types from the grammar's `src/node-types.json`.
4. Register the module in `src/lang/mod.rs` (`pub mod <lang>;` and add a
   `Box::new(<lang>::<Lang>)` entry in `supported_languages()`).
