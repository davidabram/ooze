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
| `--limit`              | Cap candidates for a quick smoke run. Applied *after* ranking, so it selects the top-ranked mutants. |
| `--strategy`           | Ordering: `discovery`, `actionable`, etc.                            |
| `--seed`               | Deterministically rank candidates for reproducible selection (see below). |
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

C#'s operator set covers boolean literal swaps, returned-boolean flips
(`return_boolean`), equality negation (`==`/`!=`), comparison boundary and
comparison negation swaps (`<`, `<=`, `>`, `>=`), logical `&&`/`||` swaps,
binary arithmetic swaps (`swap_arithmetic`: `+`/`-`, `*`/`/`, `%` → `*`;
unary `+x`/`-x` are not matched), compound assignment swaps
(`swap_assignment`: `+=`/`-=`, `*=`/`/=`; plain `=` and `%=` are excluded),
and unary mutations (`remove_not`: `!x` → `x`, `remove_unary_minus`: `-x` →
`x`, `plus_to_minus`: `+x` → `-x`). Null checks are covered by
`negate_equality` (`x == null` → `x != null`); there is no separate
null-check operator, since it would only duplicate those mutants.

C#-specific operators, enabled by default: postfix null-forgiving removal
(`null_forgiving_removal`: `value!` → `value`; the postfix `!` is never
confused with prefix logical not), is-pattern negation
(`is_pattern_negation`: `x is P` ↔ `x is not P`, covering type checks,
`is null`/`is not null`, and relational patterns like `is > 0`; `x == null`
stays `negate_equality`'s job), and checked/unchecked swaps
(`checked_unchecked_swap`: `checked(a + b)` ↔ `unchecked(a + b)`, plus the
`checked { ... }`/`unchecked { ... }` block statement forms).

0/1 integer swaps (`integer_zero_one`), string-emptying
(`string_empty_literal`: `"hello"` → `""`, regular string literals only —
verbatim, raw, and interpolated strings are never matched), null-coalescing
fallback removal (`nullish_coalescing_removal`: `a ?? b` → `a`), ternary arm
swaps (`ternary_arm_swap`: `c ? a : b` → `c ? b : a`), and ternary condition
negation (`ternary_condition_negation`: `c ? a : b` → `!(c) ? a : b`;
`if` statements are never matched) are registered but disabled by default,
matching the other languages; enable them with `--operators` or
`[mutation].operators`. Four C#-specific operators are also disabled by
default: nullable member access removal
(`nullable_access_to_member_access`: `user?.Name` → `user.Name`, `?[` too —
it can create many runtime null-reference mutants), safe-to-direct cast
(`as_expression_to_direct_cast`: `value as T` → `(T)value` — direct casts
may throw), throw-expression-to-null (`throw_expression_to_null`:
`x ?? throw new ArgumentNullException(...)` → `x ?? null`; throw
*statements* are never matched), and default-literal-to-null
(`default_literal_to_null`: `default` → `null`, bare literal only —
`default(T)` is never matched, and the mutant is invalid in non-nullable
value-type contexts). Operators only match syntax nodes, so `==` in a
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
| C#                    | yes    | yes     | 23            | no           |

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

## Deterministic selection with `--seed`

A seed deterministically ranks the mutation candidates for the current commit.
Reusing the same commit, source state, configuration, operators, seed, and
limit selects the same mutations.

```text
same commit + same config + same operators + same seed + same limit
= same selected mutants
```

How it works: ooze first discovers the **complete** candidate set — the seed
never changes which mutants exist, only their order. It then gives every
candidate a stable identity (repo-relative file path, operator, source byte
range, and the exact original/replacement text) and hashes

```text
BLAKE3("ooze-seeded-selection-v1", commit_hash, seed, stable_candidate_id)
```

into a 256-bit ranking key. Candidates are sorted ascending by that key (the
stable id breaks ties), and `--limit` is applied last. Because ranking is
independent per candidate, **increasing the limit preserves the previous
selection as a prefix**:

```bash
ooze test-mutants --seed 42 --limit 3  -- cargo test   # C, F, A
ooze test-mutants --seed 42 --limit 5  -- cargo test   # C, F, A, E, B
```

Different seeds usually produce a different order:

```bash
ooze test-mutants --seed 42 --limit 20 -- cargo test
ooze test-mutants --seed 43 --limit 20 -- cargo test
```

Seeds are **not** a partition: two different seeds may select overlapping
mutants, and there is no guarantee that they carve the candidate set into
disjoint slices.

Notes and guarantees:

- Arbitrary seed values are accepted, including any `u64` (`--seed 42`).
- The order is independent of discovery order, worker/job count, filesystem
  iteration order, and `HashMap` iteration order.
- The seed affects **only** ordering and selection. It never changes mutation
  discovery, mutation contents, test execution, timeouts, or how a probe result
  is classified.
- The current Git commit hash is mixed into the ranking, so the same seed
  selects differently across commits. Outside a Git repository (or a repo with
  no commits), the commit component is empty and the seed still works — it just
  is not pinned to a revision. ooze never substitutes the current time or random
  entropy.
- A dirty working tree still uses the committed `HEAD` hash. Reproducibility is
  defined against the **source state**: a seed reproduces a selection only when
  the tree and every other selection input are unchanged.
- Parallel execution may *finish* mutants in a different order than the plan.
  The deterministic plan order is recorded as `plan_index` in `plan.json`; do
  not confuse completion order with plan order.
- The plan and run metadata record `seed`, `selection_algorithm`
  (`hash-rank-v1`), `candidate_count`, and `selected_count`; each planned
  mutant also carries its `plan_index`, `stable_id`, and `ranking_key`.
- The `ooze-seeded-selection-v1` prefix versions the algorithm: if the ranking
  ever changes, the prefix is bumped so old seeds keep a well-defined meaning
  rather than silently selecting different mutants.

Set a seed permanently in `ooze.toml` under `[mutation]`; a CLI `--seed`
overrides it (see [configuration](config.md)).

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
