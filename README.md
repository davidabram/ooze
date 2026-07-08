# ooze

Multi-language mutation testing. Scan a repo, mutate code, run your tests in
isolated workspaces, and report which mutations your suite failed to catch.

Mutations that **survive** your tests point at code paths your tests don't
actually exercise — useful signal even when coverage looks fine.

## Languages

Support comes in two tiers — parsing a language is not the same as mutating it:

- **Mutation** (scan + mutation operators):
  - `mutate_stable` (golden-tested): **Rust**
  - `mutate_experimental`: **JavaScript · TypeScript · Python · Go · C#**
- **Scan-only** (function/branch discovery and CRAP scoring, no mutators yet):
  Bash · C · C++ · Dart · Elixir · Erlang · Gleam · Haskell · Java ·
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
| `doctor`        | Diagnose repo, config, and runtime preconditions.           |

Everything after `--` on `test-mutant(s)` is the probe command.

`ooze doctor --operators` additionally shows operator support for the detected
language(s): which operators exist, which are enabled by default, which are
available but disabled by default (e.g. the noisy `integer_zero_one`), and the
`--operators` flag to include the disabled ones.

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

## Quick start (Go)

```bash
./target/release/ooze test-mutants --preset go
```

Go ships an initial (experimental) mutation operator set: boolean swaps
(`true`/`false`), equality negation (`==`/`!=`), comparison boundary swaps
(`<`/`<=`, `>`/`>=`), 0/1 integer swaps, and logical `&&`/`||` swaps. Operators
only match real syntax nodes, so `==` in a comment or string never mutates.

The `go` preset fills any options you left unset with good Go defaults: the
`worktree` backend, warmup, `go test ./...` as the probe, a shared
`GOCACHE={build_cache}/go-build` (Go's build cache is safe to share across
workers, so no per-worker split), and `GOTMPDIR={build_cache}` so probe temp
writes stay out of the system `/tmp`. As with every preset, explicit CLI flags
and `ooze.toml` values win over the preset's defaults, and `ooze doctor` shows
which fills are active or overridden:

```bash
./target/release/ooze test-mutants --preset go -- go test ./pkg/foo
```

## Quick start (Node / JavaScript / TypeScript)

```bash
./target/release/ooze test-mutants --preset node
```

The `node` preset requires a `package.json` at the project path and picks the
package manager from the lockfile it finds (priority `bun` > `pnpm` > `yarn` >
`npm`; a bare `package.json` means npm). That choice drives both the default
probe (`bun test`, `pnpm test`, `yarn test`, or `npm test`) and the cache
envs, which point the package-manager cache into the shared build-cache dir:

| Lockfile                 | Probe       | Cache envs                                                             |
| ------------------------ | ----------- | ---------------------------------------------------------------------- |
| `bun.lockb` / `bun.lock` | `bun test`  | `BUN_INSTALL_CACHE_DIR={build_cache}/bun`                               |
| `pnpm-lock.yaml`         | `pnpm test` | `npm_config_cache={build_cache}/npm`, `PNPM_HOME={build_cache}/pnpm-home` |
| `yarn.lock`              | `yarn test` | `YARN_CACHE_FOLDER={build_cache}/yarn`                                  |
| `package-lock.json`      | `npm test`  | `npm_config_cache={build_cache}/npm`                                    |

Like Go, Node keeps a shared cache (no `--per-worker-cache`): package-manager
caches are safe to share across workers, while each workspace stays isolated
by the `worktree` backend. As with every preset, explicit CLI flags and
`ooze.toml` values win over the preset's defaults, and `ooze doctor` shows
which fills are active or overridden:

```bash
./target/release/ooze test-mutants --preset node -- npm test -- --runInBand
```

## Quick start (Python)

```bash
./target/release/ooze test-mutants --preset python
```

The `python` preset applies when at least one of `pyproject.toml`, `setup.py`,
`setup.cfg`, or `requirements.txt` exists at the project path. It fills any
options you left unset with: the `worktree` backend, warmup, `pytest` as the
probe, and three env defaults that keep per-mutant state out of the workspace:

- `PYTHONPYCACHEPREFIX={build_cache}/pycache` — `.pyc` bytecode is written to
  the shared build-cache dir, never the checkout.
- `PYTEST_ADDOPTS=--cache-clear` — pytest's own cache can't carry `--lf`-style
  state from one mutant to the next.
- `TMPDIR={build_cache}/tmp` — probe temp files stay out of the system `/tmp`.

Like Go and Node, Python keeps a shared cache root (no `--per-worker-cache`).
As with every preset, explicit CLI flags and `ooze.toml` values win over the
preset's defaults, and `ooze doctor` shows which fills are active or
overridden:

```bash
./target/release/ooze test-mutants --preset python -- pytest tests/unit
```

## Quick start (C# / .NET)

```bash
./target/release/ooze test-mutants --preset csharp
```

The `csharp` preset applies when at least one `*.sln` or `*.csproj` file
exists at the project path (checked non-recursively). It fills any options
you left unset with: the `worktree` backend, warmup, `dotnet test` as the
probe, `DOTNET_CLI_TELEMETRY_OPTOUT=1` (quiet, network-free probe runs), and
`NUGET_PACKAGES={build_cache}/nuget` (the NuGet global packages folder is
concurrency-safe, so workers share it while build outputs stay inside each
isolated workspace — no `--per-worker-cache`).

The C# operator set covers boolean literal swaps, returned-boolean flips,
equality negation (`==`/`!=`), comparison boundary and comparison negation
swaps (`<`, `<=`, `>`, `>=`), logical `&&`/`||` swaps, binary arithmetic
swaps (`+`/`-`, `*`/`/`, `%` → `*`), compound assignment swaps (`+=`/`-=`,
`*=`/`/=`), and unary mutations (`!x` → `x`, `-x` → `x`, `+x` → `-x`).
C#-specific operators cover null-forgiving removal (`value!` → `value`),
is-pattern negation (`x is P` ↔ `x is not P`, including `is null` and
relational patterns like `is > 0`), and checked/unchecked swaps
(`checked(a + b)` ↔ `unchecked(a + b)`, block forms too).
Null checks mutate via equality negation (`x == null` → `x != null`); 0/1
integer swaps, string-emptying (`"hello"` → `""`), null-coalescing fallback
removal (`a ?? b` → `a`), ternary arm swaps (`c ? a : b` → `c ? b : a`),
ternary condition negation (`c ? a : b` → `!(c) ? a : b`), nullable member
access removal (`user?.Name` → `user.Name`), safe-to-direct cast
(`value as T` → `(T)value`), throw-expression-to-null (`x ?? throw ...` →
`x ?? null`), and default-literal-to-null (`default` → `null`) are available
but disabled by default (enable with `--operators`, e.g. `--operators
ternary_arm_swap`). Operators only match real syntax nodes, so `==` in a
comment or string never mutates.
As with every preset, explicit CLI flags and `ooze.toml` values win over the
preset's defaults, and `ooze doctor` shows which fills are active or
overridden:

```bash
./target/release/ooze test-mutants --preset csharp -- dotnet test Some.Tests.csproj
```

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
| `--preset`             | Language preset filling unset options with ecosystem defaults. `rust`: worktree backend, per-worker cache, warmup, `CARGO_TARGET_DIR={build_cache}`, probe `cargo test`. `go`: worktree backend, warmup, shared `GOCACHE={build_cache}/go-build`, `GOTMPDIR={build_cache}`, probe `go test ./...`. `node`: worktree backend, warmup, shared package-manager cache envs under `{build_cache}`, probe from lockfile detection (`bun`/`pnpm`/`yarn`/`npm test`). `python`: worktree backend, warmup, `PYTHONPYCACHEPREFIX={build_cache}/pycache`, `PYTEST_ADDOPTS=--cache-clear`, `TMPDIR={build_cache}/tmp`, probe `pytest`. |
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
