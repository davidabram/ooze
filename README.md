# ooze

Multi-language mutation testing. Scan a repo, mutate code, run your tests in
isolated workspaces, and report which mutations your suite failed to catch.

Mutations that **survive** your tests point at code paths your tests don't
actually exercise — useful signal even when coverage looks fine.

## Languages

Support comes in two tiers — parsing a language is not the same as mutating it:

- **Mutation** (scan + mutation operators):
  - `mutate_stable` (golden-tested): **Rust**
  - `mutate_experimental`: **JavaScript · TypeScript · Python**
- **Scan-only** (function/branch discovery and CRAP scoring, no mutators yet):
  Bash · C · C++ · C# · Dart · Elixir · Erlang · Gleam · Go · Haskell · Java ·
  Julia · Lua · OCaml · PHP · Ruby · Scala · Swift · Zig.

Run `ooze languages` for the live list with each language's support level and
operator count (`--format json` for machine output). Operators live per language
in `src/lang/`.

## Install

```bash
cargo build --release
# binary: ./target/release/ooze
```

## Commands

| Command         | What it does                                                |
| --------------- | ----------------------------------------------------------- |
| `scan`          | List function spans across the repo.                        |
| `crap`          | Score functions by the CRAP formula (optionally with lcov). |
| `mutants`       | Print mutation candidates (JSON).                           |
| `operators`     | List mutation operators and their metadata.                 |
| `languages`     | List supported languages and their support level.           |
| `apply-mutant`  | Apply one mutation in a workspace and print the diff.       |
| `test-mutant`   | Apply one mutation, run a probe, classify the outcome.      |
| `test-mutants`  | Run a batch in parallel and emit a summary report.          |
| `warmup`        | Pre-build the probe in the shared build cache dir.          |

Everything after `--` on `test-mutant(s)` is the probe command.

## Quick start (Rust)

```bash
./target/release/ooze test-mutants --preset rust
```

The `rust` preset fills any options you left unset with good Rust defaults:
the `worktree` backend, per-worker `CARGO_TARGET_DIR` build caches, warmup,
and `cargo test` as the probe. Explicit CLI flags and `ooze.toml` values
always override preset defaults, so this uses your probe, not the default:

```bash
./target/release/ooze test-mutants --preset rust -- cargo test --lib
```

The preset is shorthand for (approximately):

```bash
./target/release/ooze test-mutants \
  --path . \
  --jobs 4 \
  --timeout-seconds 180 \
  --workspace-backend worktree \
  --per-worker-cache \
  --warmup \
  --probe-env CARGO_TARGET_DIR={build_cache} \
  -- cargo test
```

- `--per-worker-cache` gives each worker its own `build-cache-job-{i}` so
  parallel runs reuse incremental builds instead of fighting over one
  build directory.
- `--warmup` pre-builds the probe in each worker dir; first mutant per worker
  isn't a cold compile. Doubles as a baseline check (warmup fails → batch
  aborts).

Git worktrees (recommended inside a Git repo):

```bash
./target/release/ooze test-mutants \
  --workspace-backend worktree \
  --jobs 4 \
  --per-worker-cache \
  --warmup \
  --probe-env CARGO_TARGET_DIR={build_cache} \
  -- cargo test
```

The worktree backend creates one Git worktree per worker and reuses it across
mutants. It is rootless, CI-friendly, and a good default for most projects —
`--workspace-backend auto` picks it automatically inside a Git repository.
Requires running inside a Git repository; mutants are applied against `HEAD`,
so commit your changes first. Worktrees live under `.ooze/runs/worktrees` and
are removed at the end of the run; only paths under that directory are cleaned
destructively.

Linux + overlayfs (no full repo copy per mutant; needs root):

```bash
sudo ./target/release/ooze test-mutants \
  --path . \
  --strategy actionable \
  --workspace-backend overlay \
  --jobs 4 --timeout-seconds 180 \
  --per-worker-cache --warmup \
  --probe-env CARGO_TARGET_DIR={build_cache} \
  -- cargo test
```

## Other languages

Use `--probe-env KEY=VALUE` to give each worker its own build cache. `{worker}`
expands to the worker index; path-like values are auto-created.

```bash
# Go
--probe-env GOCACHE=.ooze/cache/gocache-{worker} -- go test ./...

# Gradle
--probe-env GRADLE_USER_HOME=.ooze/cache/gradle-{worker} -- ./gradlew test --no-daemon

# npm
--probe-env npm_config_cache=.ooze/cache/npm-{worker} -- npm test

# pytest
--probe-env PYTEST_CACHE_DIR=.ooze/cache/pytest-{worker} -- pytest -q
```

Full per-language recipes in [docs/running-mutants.md](docs/running-mutants.md).

## Useful flags on `test-mutants`

| Flag                   | Purpose                                                              |
| ---------------------- | -------------------------------------------------------------------- |
| `--jobs N`             | Parallel workers.                                                    |
| `--limit N`            | Cap candidates (smoke runs).                                         |
| `--strategy`           | `discovery`, `actionable`, ...                                       |
| `--changed-only BASE`  | Only mutate files changed vs `BASE` (e.g. `main`). For PR/CI runs.   |
| `--timeout-seconds`    | Per-mutant probe timeout (→ `timeout` verdict).                      |
| `--preset`             | Language preset filling unset options with ecosystem defaults. `rust`: worktree backend, per-worker cache, warmup, `CARGO_TARGET_DIR={build_cache}`, probe `cargo test`. |
| `--workspace-backend`  | `copy`, `overlay`, `worktree`, `auto` (worktree in a Git repo, else copy). |
| `--exclude`            | Extra globs. Defaults + `.gitignore` always apply.                   |
| `--coverage`           | Feed coverage into ordering. `format:path` or a bare path to auto-detect. Formats: `lcov`, `cobertura`, `jacoco`, `go-cover`. Repeatable; reports are merged. |
| `--lcov`               | Deprecated alias for `--coverage lcov:<path>`.                       |
| `--warmup`             | Pre-build probe per worker.                                          |
| `--per-worker-cache`   | Per-worker `build-cache-job-{i}` dirs.                               |
| `--probe-env KEY=VAL`  | Env vars on probe + warmup. `{worker}` → worker index, `{build_cache}` → build cache path. |
| `--cache-dir`          | Where caches live (default `.ooze/cache`).                           |
| `--runs-dir`           | Where workspaces live (default `.ooze/runs`).                        |

## PR / CI runs (`--changed-only`)

Mutate only the files a branch touched instead of the whole repo:

```bash
./target/release/ooze test-mutants --path . --changed-only main -- cargo test
```

The changed set is the union of `git diff --name-only main...HEAD` (commits on the
branch since its merge-base with `main`), uncommitted working-tree changes, and
untracked-but-not-ignored files — so it works the same in CI and during local
iteration. Candidates in unchanged files are dropped before scheduling, which keeps
PR runs fast. Also settable as `changed_only = "main"` under `[scope]` in `ooze.toml`,
and supported on `plan-mutants` for previewing the selection.

## Output

`test-mutants` writes a JSON report to stdout:

```bash
./target/release/ooze test-mutants --path . -- <your-test-command> > report.json
jq '.summary' report.json
jq '.outcomes[] | select(.status == "survived")' report.json
```

Verdicts: `killed`, `survived`, `timeout`, `error`.

### Report size

Reports can grow large because every outcome carries a diff plus probe
stdout/stderr and source context. Trim them with `--report-detail` or the
per-field flags:

| Flag                    | Effect                                              |
|-------------------------|-----------------------------------------------------|
| `--report-detail LEVEL` | `compact` (survivors only, no diffs/output), `normal` (diffs, no probe output), or `full` (everything). |
| `--no-diff`             | Drop unified diffs.                                 |
| `--no-stdout`           | Drop probe stdout.                                  |
| `--no-stderr`           | Drop probe stderr.                                  |
| `--only-survivors`      | Keep only survived mutants in `outcomes`.           |

Defaults are per format: `human`, `agent-tasks-*`, `sarif`, and
`github-annotations` use `compact`; `json` uses `normal`. The per-field flags
compose on top of the chosen level, and summary counts and exit codes are
unaffected by trimming. All of these are also settable under `[report]` in
`ooze.toml`.

## Defaults

- Always excluded: `target/**`, `.ooze/**`, `.git/**`, `node_modules/**`, `vendor/**`, `__pycache__/**`, `.gradle/**`.
- `.gitignore` entries are merged into excludes automatically.
- Workspaces under `.ooze/runs/`, caches under `.ooze/cache/`.

## Docs

- [docs/running-mutants.md](docs/running-mutants.md) — per-language recipes.
- [docs/cyclomatic-conventions.md](docs/cyclomatic-conventions.md) — how
  cyclomatic complexity is counted per language.
- [docs/deferred-languages.md](docs/deferred-languages.md) — languages on the
  backlog.
