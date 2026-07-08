# Running `test-mutants`

`test-mutants` applies each mutation candidate inside an isolated workspace and
runs your probe (the test command after `--`). The result of the probe decides
the verdict:

- exit 0 → `survived` (your tests didn't notice the mutation)
- non-zero exit → `killed`
- timeout → `timeout`

## Common flags

| Flag                   | Purpose                                                              |
| ---------------------- | -------------------------------------------------------------------- |
| `--path`               | Repo root to scan and mutate.                                        |
| `--jobs`               | Parallel worker count.                                               |
| `--limit`              | Cap candidates for a quick smoke run.                                |
| `--strategy`           | Ordering: `discovery`, `actionable`, etc.                            |
| `--timeout-seconds`    | Per-mutant probe timeout.                                            |
| `--preset`             | Language preset that fills unset options with ecosystem defaults (see below). `rust`, `go`, `node`, and `python` for now. |
| `--workspace-backend`  | `worktree` (Git, rootless), `copy` (portable), `overlay` (Linux; needs root), `auto` (worktree in a Git repo, else copy). |
| `--exclude`            | Extra glob excludes, comma-separated. Defaults + `.gitignore` apply. |
| `--warmup`             | Pre-build the probe in each worker dir before running mutants.       |
| `--per-worker-cache`   | Give each worker its own build cache dir (avoids build lock churn).  |
| `--probe-env KEY=VAL`  | Set env vars on probe + warmup. `{worker}` → worker index, `{build_cache}` → build cache path. |

Everything after `--` is the probe command line.

## Presets

`--preset rust` fills every runner option you left unset with good Rust
defaults:

- `--workspace-backend worktree`
- `--per-worker-cache`
- `--warmup`
- `--probe-env CARGO_TARGET_DIR={build_cache}` (skipped if you already set
  `CARGO_TARGET_DIR`)
- probe `cargo test` (only when no probe is given after `--` and none is set
  in `ooze.toml`)

The preset never enables `sccache` automatically — the same command expands
the same way on every machine. If you want it, opt in explicitly (ooze doctor
suggests this when it finds sccache):

```bash
ooze test-mutants --preset rust --probe-env RUSTC_WRAPPER=sccache
```

`--preset go` does the same for Go modules:

- `--workspace-backend worktree`
- `--warmup`
- `--probe-env GOCACHE={build_cache}/go-build` (skipped if you already set
  `GOCACHE`)
- `--probe-env GOTMPDIR={build_cache}` (skipped if you already set `GOTMPDIR`)
- probe `go test ./...` (only when no probe is given after `--` and none is
  set in `ooze.toml`)

Unlike the Rust preset, Go keeps the default shared build cache instead of
`--per-worker-cache`: Go's build cache is concurrency-safe by design, so all
workers share one `GOCACHE`. `GOTMPDIR` points at the same shared dir — the
`go` command creates a unique work dir per invocation inside it — keeping
probe temp writes out of the system `/tmp`.

`--preset node` does the same for JavaScript/TypeScript projects. It requires
a `package.json` at the project path and detects the package manager from the
lockfile, with priority `bun` > `pnpm` > `yarn` > `npm` when several coexist
(a bare `package.json` means npm). The detected package manager drives the
probe and the cache envs:

- `--workspace-backend worktree`
- `--warmup`
- probe `bun test` / `pnpm test` / `yarn test` / `npm test` (only when no
  probe is given after `--` and none is set in `ooze.toml`)
- cache envs, skipped per key if you already set them:
  - bun: `BUN_INSTALL_CACHE_DIR={build_cache}/bun`
  - pnpm: `npm_config_cache={build_cache}/npm` and
    `PNPM_HOME={build_cache}/pnpm-home`
  - yarn: `YARN_CACHE_FOLDER={build_cache}/yarn`
  - npm: `npm_config_cache={build_cache}/npm`

Like Go, Node keeps a shared cache rather than `--per-worker-cache`:
package-manager caches are safe to share across workers, while each worker's
workspace stays isolated by the worktree backend.

`--preset python` covers Python projects. It applies when at least one of
`pyproject.toml`, `setup.py`, `setup.cfg`, or `requirements.txt` exists at
the project path, and fills:

- `--workspace-backend worktree`
- `--warmup`
- probe `pytest` (only when no probe is given after `--` and none is set in
  `ooze.toml`)
- env defaults, skipped per key if you already set them:
  - `PYTHONPYCACHEPREFIX={build_cache}/pycache` — `.pyc` bytecode lands in
    the shared build-cache dir instead of the workspace, so mutants never
    run against stale bytecode from the checkout
  - `PYTEST_ADDOPTS=--cache-clear` — pytest's cache can't carry state
    between mutants
  - `TMPDIR={build_cache}/tmp` — probe temp files stay out of the system
    `/tmp`

Python also keeps a shared cache root rather than `--per-worker-cache`.

`--preset csharp` covers C#/.NET projects. It applies when at least one
`*.sln` or `*.csproj` file exists at the project path (checked
non-recursively), and fills:

- `--workspace-backend worktree`
- `--warmup`
- probe `dotnet test` (only when no probe is given after `--` and none is
  set in `ooze.toml`)
- env defaults, skipped per key if you already set them:
  - `DOTNET_CLI_TELEMETRY_OPTOUT=1` — keeps probe runs quiet and
    network-free
  - `NUGET_PACKAGES={build_cache}/nuget` — the NuGet global packages folder
    is concurrency-safe, so all workers share it while build outputs stay
    inside each isolated workspace

C# also keeps a shared cache rather than `--per-worker-cache`. As with every
preset, explicit CLI flags and `ooze.toml` values win over the preset's
defaults, and `ooze doctor` shows which fills are active or overridden.

C#'s initial operator set covers boolean literal swaps, equality negation
(`==`/`!=`), comparison boundary and comparison negation swaps (`<`, `<=`,
`>`, `>=`), and logical `&&`/`||` swaps. 0/1 integer swaps
(`integer_zero_one`) are registered but disabled by default, like every other
language; enable them with `--operators integer_zero_one` or
`[mutation].operators`. Operators only match syntax nodes, so `==` in a
comment or string literal never mutates.

## Preset and operator coverage

Where each preset language stands (operator counts are registered mutation
operators; `ooze operators` lists them all, `ooze languages` shows support
levels):

| Language              | Preset | Scanner | Operators     | E2E verified |
| --------------------- | ------ | ------- | ------------- | ------------ |
| Rust                  | yes    | yes     | 23            | yes          |
| Go                    | yes    | yes     | 5 (baseline)  | yes          |
| JavaScript/TypeScript | yes    | yes     | 18            | yes          |
| Python                | yes    | yes     | 20            | yes          |
| C#                    | yes    | yes     | 6 (baseline)  | no           |

The baseline operator set every mutating language covers: boolean literal swap,
equality negation, comparison boundary, logical and/or swap, and integer 0/1
swap. Note `integer_zero_one` is `default_enabled: false` in every language
(it tends to be noisy); enable it explicitly with
`--operators integer_zero_one` or `[mutation].operators`.

To see the same information scoped to your project, run:

```bash
ooze doctor --operators
```

It reads the mutator registry for the detected language(s) and shows which
operators are available, which are enabled by default, which are available
but disabled by default, and the `--operators` flag to include the disabled
ones. Mixed projects get one section per detected language; `--format json`
includes the same data under an `operators` key.

Presets are default-fillers, not overrides: explicit CLI flags and `ooze.toml`
values always win. The applied fills are printed on stderr as
`ooze: preset <name>: ...` so the expansion stays visible. `ooze doctor` shows
the same fill list for the preset it recommends, marking fills your
`ooze.toml` already overrides.

```bash
# everything defaulted
ooze test-mutants --preset rust

# explicit probe wins over the preset's `cargo test`
ooze test-mutants --preset rust -- cargo test --lib

# explicit backend wins over the preset's worktree
ooze test-mutants --preset rust --workspace-backend overlay
```

The `rust` preset requires a `Cargo.toml` at the project path and the `go`
preset a `go.mod`; both default to the worktree backend, which requires
running inside a Git repository (you'll get a clear error otherwise; pass
`--workspace-backend copy` to opt out).

## Workspace backends

- `worktree` — creates one Git worktree per worker and reuses it across
  mutants (reset with `git reset --hard` + `git clean -fdx` between mutants).
  Rootless, CI-friendly, and a good default for most projects. Requires
  running inside a Git repository, and mutants are applied against `HEAD`, so
  commit your changes first. Worktrees live under `.ooze/runs/worktrees` and
  are removed when the run finishes; only paths under that directory are
  cleaned destructively.
- `copy` — copies the repo into a temp dir per mutant. Portable, works
  anywhere, slowest for large repos. Automatically attempts reflink /
  copy-on-write file cloning when supported by the filesystem, and falls
  back to regular copying otherwise.
- `overlay` — OverlayFS mount per mutant. Linux only and needs root; never
  chosen automatically.
- `auto` — `worktree` inside a Git repository, otherwise `copy`.

```bash
./target/release/ooze test-mutants \
  --workspace-backend worktree \
  --jobs 4 \
  --per-worker-cache \
  --warmup \
  --probe-env CARGO_TARGET_DIR={build_cache} \
  -- cargo test
```

## Rust (cargo)

Use `--per-worker-cache` so each worker gets its own `.ooze/cache/build-cache-job-{i}`
and incremental builds are reused mutant-to-mutant. Wire `CARGO_TARGET_DIR` to
that dir via `--probe-env` so cargo actually uses it:

```bash
sudo ./target/release/ooze test-mutants \
  --path . \
  --strategy actionable \
  --workspace-backend overlay \
  --limit 10 \
  --jobs 4 \
  --timeout-seconds 180 \
  --per-worker-cache \
  --warmup \
  --probe-env CARGO_TARGET_DIR={build_cache} \
  --exclude "tests/fixtures/**,examples/**" \
  -- cargo test
```

Drop `sudo` and switch to `--workspace-backend copy` if you don't want
overlayfs.

## Go

Go has initial mutation operator support (`mutate_experimental`). The first
operator set sticks to swaps that always keep the code compiling:

- boolean swaps (`true` ↔ `false`)
- equality negation (`==` ↔ `!=`)
- comparison boundary swaps (`<` ↔ `<=`, `>` ↔ `>=`)
- 0/1 integer swaps (`0` ↔ `1`)
- logical swaps (`&&` ↔ `||`)

Candidates come from tree-sitter syntax nodes, so operator-like text inside
comments and string literals is never mutated.

Manual smoke run against any Go module (the CI equivalent lives in
`tests/cli.rs` and skips when `go` is not installed):

```bash
ooze test-mutants --path <go-module> --preset go --limit 5 --jobs 2
```

This discovers mutants, uses the worktree backend, runs `go test ./...` per
mutant, and reports killed/survived/timeout/error in the usual formats.

Go's build/test cache lives in `GOCACHE`. Give each worker its own:

```bash
./target/release/ooze test-mutants \
  --path . \
  --jobs 4 \
  --timeout-seconds 180 \
  --warmup \
  --probe-env GOCACHE=.ooze/cache/gocache-{worker} \
  --probe-env GOFLAGS=-count=1 \
  -- go test ./...
```

`{worker}` expands to the worker index; the path is pre-created automatically.

## Java / Kotlin (Gradle)

Isolate Gradle's user home per worker so the daemon and caches don't collide:

```bash
./target/release/ooze test-mutants \
  --path . \
  --jobs 4 \
  --timeout-seconds 300 \
  --warmup \
  --probe-env GRADLE_USER_HOME=.ooze/cache/gradle-{worker} \
  -- ./gradlew test --no-daemon
```

For Maven, use `MAVEN_OPTS` or `-Dmaven.repo.local`:

```bash
--probe-env MAVEN_OPTS="-Dmaven.repo.local=.ooze/cache/m2-{worker}"
-- mvn -q test
```

## Node.js (npm / pnpm / yarn)

Point package-manager caches at per-worker dirs:

```bash
./target/release/ooze test-mutants \
  --path . \
  --jobs 4 \
  --warmup \
  --probe-env npm_config_cache=.ooze/cache/npm-{worker} \
  -- npm test --silent
```

For pnpm: `--probe-env PNPM_HOME=.ooze/cache/pnpm-{worker}`.
For yarn berry: `--probe-env YARN_GLOBAL_FOLDER=.ooze/cache/yarn-{worker}`.

## Python (pytest)

Python doesn't have a heavy build cache, but pytest's collection cache and
`__pycache__` can race across workers:

```bash
./target/release/ooze test-mutants \
  --path . \
  --jobs 4 \
  --probe-env PYTHONDONTWRITEBYTECODE=1 \
  --probe-env PYTEST_CACHE_DIR=.ooze/cache/pytest-{worker} \
  -- pytest -q
```

## Ruby (Bundler)

```bash
--probe-env BUNDLE_PATH=.ooze/cache/bundle-{worker}
-- bundle exec rake test
```

## Generic recipe

1. Identify the cache env var(s) for your toolchain.
2. Add `--probe-env KEY=.ooze/cache/<name>-{worker}` for each.
3. Add `--warmup` so the first mutant per worker isn't a cold build.
4. Pick `--jobs` based on cores and how heavy your probe is.

Two tokens are expanded in `--probe-env` values:
- `{worker}` — the worker index (0-based).
- `{build_cache}` — the path to the worker's build cache dir (set by `--per-worker-cache` or `--build-cache-dir`).
