# ooze configuration

ooze reads `ooze.toml` from the repo root if present. CLI flags always
override config values. A canonical example lives at
[`ooze.example.toml`](../ooze.example.toml) — copy it to `ooze.toml` and
edit to taste, or run `ooze init` to generate one in place.

Unknown keys are rejected (the config uses `deny_unknown_fields`), so a
typo will surface as a load error rather than being silently ignored.

## Sections

### `[scope]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `exclude` | `[string]` | `[]` | Extra exclude globs layered on top of `DEFAULT_EXCLUDES` (`.git`, `target`, `.ooze`) and `.gitignore`. |

### `[mutation]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `strategy` | `string` | `"actionable"` | Mutation selection strategy. |
| `operators` | `[string]` | all defaults-on | Explicit allow-list. If set, only these operators run. |
| `exclude_operators` | `[string]` | `[]` | Operators to drop from the active set. |
| `static_skips` | `bool` | `true` | Skip mutants that the static analyzer flags as equivalent. |
| `context_lines` | `int` | `3` | Diff context lines shown around each mutant. |
| `limit` | `int` | unlimited | Cap on the number of mutants to run. |
| `coverage` | `[string]` | `[]` | Coverage reports used to prioritize by coverage. Each entry is `format:path` (`lcov`, `cobertura`, `jacoco`, `go-cover`) or a bare path to auto-detect; multiple entries are merged. |
| `lcov` | `path` | none | Deprecated alias for `coverage = ["lcov:<path>"]`. |

Built-in operators: `comparison_boundary`, `comparison_negation`,
`negate_equality`, `swap_logical`, `swap_boolean`, `remove_not`,
`return_boolean`, `integer_zero_one`. Rust additionally has
`range_inclusive_exclusive` (`..` <-> `..=`), `swap_predicate_method`
(`is_some` <-> `is_none`, `is_ok` <-> `is_err`), and
`negate_predicate_method` (`is_empty()` -> `!is_empty()`,
`contains(x)` -> `!contains(x)`), `iterator_any_all` (`any(..)` <-> `all(..)`),
`match_bool_pattern` (`true =>` <-> `false =>`), `ok_err_boolean`
(`Ok(true)` <-> `Ok(false)`, `Err(..)` likewise), `some_boolean`
(`Some(true)` <-> `Some(false)`), `option_some_none` (`Some(x)` -> `None`),
`remove_try` (`foo()?` -> `foo()`), `unwrap_to_unwrap_or_default`
(`x.unwrap()` -> `x.unwrap_or_default()`), `min_max_swap` (`min` <-> `max`),
`match_wildcard_to_panic` (`_ => expr` -> `_ => panic!(..)`), `empty_vec_macro`
(`vec![a, b, c]` -> `vec![]`), `saturating_checked_swap` (`checked_add` <->
`saturating_add`, `checked_sub` <-> `saturating_sub`), and
`expect_to_unwrap_or_default` (`x.expect("..")` -> `x.unwrap_or_default()`). All
are on by default except `integer_zero_one`, `option_some_none`, `remove_try`,
`unwrap_to_unwrap_or_default`, `min_max_swap`, `match_wildcard_to_panic`,
`empty_vec_macro`, `saturating_checked_swap`, and `expect_to_unwrap_or_default`,
which are off by default and must be opted in via `operators`. An operator only
runs for languages that implement it.

`operators` / `exclude_operators` filter by **semantic operator**, so they apply
to every language being scanned. Each operator is realized by one or more
per-language implementations in the registry (`src/mutate/registry.rs`); a
candidate's `implementation` field reports the one that produced it, e.g.
`rust.negate_equality`.

### `[runner]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `workspace_backend` | `string` | `"auto"` | How per-worker workspaces are materialized. |
| `jobs` | `int` | `2` | Parallel mutant runners. |
| `timeout_seconds` | `int` | `120` | Per-mutant wall-clock cap. |
| `preflight` | `bool` | `true` | Run the probe once before any mutant to verify the baseline is green. |
| `shared_target` | `bool` | `false` | Share `target/` across workers (faster, riskier). |
| `warmup` | `bool` | `true` | Pre-warm caches before fanning out workers. |
| `cache_dir` | `path` | `.ooze/cache` | Per-tool cache root. |
| `runs_dir` | `path` | `.ooze/runs` | Where per-run artifacts land. |
| `cargo_target_dir` | `path` | none | Override `CARGO_TARGET_DIR` for runners. |

### `[probe]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `command` | `[string]` | `["cargo", "test", "--jobs", "1"]` | Command run against each mutant. |
| `env` | `[string]` | `[]` | Extra `KEY=VALUE` entries. The literal `{worker}` is expanded to the worker index. |

### `[report]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `format` | `string` | `"human"` | One of `human`, `json`, `sarif`, etc. |
| `output` | `path` | stdout | Optional output file. |
| `fail_on_survivors` | `bool` | `true` | Non-zero exit when survivors remain. |
| `allow_incomplete` | `bool` | `false` | Treat an incomplete run as success. |

## Minimal example

```toml
[mutation]
strategy = "actionable"
context_lines = 3

[runner]
workspace_backend = "auto"
jobs = 2
timeout_seconds = 120
preflight = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["cargo", "test", "--jobs", "1"]

[report]
format = "human"
fail_on_survivors = true
allow_incomplete = false
```
