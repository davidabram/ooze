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
| `--workspace-backend`  | `copy` (portable), `overlay` (Linux, faster; needs root), `auto`.    |
| `--exclude`            | Extra glob excludes, comma-separated. Defaults + `.gitignore` apply. |
| `--warmup`             | Pre-build the probe in each worker dir before running mutants.       |
| `--no-shared-target`   | Give each worker its own cargo target dir (avoids cargo lock churn). |
| `--probe-env KEY=VAL`  | Set env vars on probe + warmup. `{worker}` expands to worker index.  |

Everything after `--` is the probe command line.

## Rust (cargo)

Per-worker cargo target dirs are first-class. Use `--no-shared-target` so each
worker gets `.ooze/cache/cargo-target-job-{i}` and incremental builds are
reused mutant-to-mutant.

```bash
sudo ./target/release/ooze test-mutants \
  --path . \
  --strategy actionable \
  --workspace-backend overlay \
  --limit 10 \
  --jobs 4 \
  --timeout-seconds 180 \
  --no-shared-target \
  --warmup \
  --exclude "tests/fixtures/**,examples/**" \
  -- cargo test
```

Drop `sudo` and switch to `--workspace-backend copy` if you don't want
overlayfs.

## Go

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

The `{worker}` token is the only template ooze substitutes; everything else is
passed through verbatim to the probe (and to warmup).
