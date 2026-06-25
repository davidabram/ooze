# Onboarding ooze into an existing project

This guide is for introducing ooze into a **brown-field** codebase without
making the team resent mutation testing. The goal of the first few weeks is
not a coverage gate — it is to surface a small number of high-value survived
mutants and turn them into concrete test-writing tasks.

Everything here uses commands, flags, and config keys that exist in ooze
today. Run `ooze --help`, `ooze <command> --help`, and `ooze operators` to
confirm the surface for your version.

## Guiding principle

Do **not** start by mutating the whole repo. Start ooze as a *test-improvement
advisor*, scoped to code the team is already changing, in advisory (non-gating)
mode. Tighten only once the signal is trusted.

## What ooze actually mutates

ooze ships a fixed set of small, high-signal operators — boolean, comparison,
equality, `None`/null, and empty-collection mutations. List them for your build
with:

```bash
ooze operators
```

There are **no** arithmetic, string, regex, or statement-removal operators, so
there is nothing of that kind to "turn off." The only default-off operator is
`integer_zero_one`; opt into it explicitly via `operators` if you want it. An
operator only runs for languages that implement it (see
[`docs/config.md`](./config.md) for the per-language breakdown).

## Phase 1 — Baseline

Confirm the existing test command is stable and ooze's preconditions pass.

```bash
ooze doctor --path .
```

Generate a starter config:

```bash
ooze init-config --language rust
```

Edit `ooze.toml` so the probe runs your normal test command:

```toml
[probe]
command = ["cargo", "test", "--jobs", "1"]

[runner]
preflight = true
timeout_seconds = 120
jobs = 1
workspace_backend = "auto"
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[report]
format = "human"
fail_on_survivors = false
allow_incomplete = false
```

Smoke-test a single mutant:

```bash
ooze test-mutants --limit 1
```

Expected: preflight passes, one mutant runs, the original tree is left
unchanged. Fix any runner/probe/config issues before widening scope.

> Note: `test-mutants` (plural) runs a batch and produces a report.
> `test-mutant` (singular) runs exactly one mutant by id — handy for
> debugging, not for normal runs.

## Phase 2 — Changed code only

For brown-field repos, scope to what the team is editing:

```toml
[scope]
changed_only = "main"
```

or per-invocation:

```bash
ooze plan-mutants --changed-only main --limit 20
```

`plan-mutants` shows the selection, scores, and applied excludes **without**
running any probes — use it to sanity-check scope before spending test time.

A good first real run:

```bash
ooze test-mutants \
  --changed-only main \
  --strategy actionable \
  --limit 10 \
  --format agent-tasks-markdown \
  --output ooze-tasks.md \
  --no-fail-on-survivors
```

This produces test-writing tasks without failing anything.

## Phase 3 — Exclude bad mutation targets

Keep ooze on business logic, not generated code, fixtures, or snapshots.
`DEFAULT_EXCLUDES` (`.git`, `target`, `.ooze`, `node_modules`, `vendor`,
`__pycache__`, `.gradle`) and `.gitignore` are always applied; add the rest:

```toml
[scope]
exclude = [
  "dist/**",
  "build/**",
  "coverage/**",
  "**/generated/**",
  "**/*.generated.*",
  "**/*_pb2.py",
  "**/*.pb.go",
  "**/fixtures/**",
  "**/testdata/**",
  "**/__snapshots__/**",
  "**/snapshots/**",
]
```

## Phase 4 — Use coverage to rank, not to punish

Feed coverage in so ooze can prioritize; don't turn it into a gate yet.
Each `--coverage` entry is `format:path` (`lcov`, `cobertura`, `jacoco`,
`go-cover`) or a bare path to auto-detect; multiple entries merge.

```bash
ooze test-mutants --coverage lcov:coverage/lcov.info
ooze test-mutants --coverage cobertura:coverage.xml
ooze test-mutants --coverage go-cover:coverage.out
ooze test-mutants --coverage jacoco:build/reports/jacoco/test/jacocoTestReport.xml
```

With the `actionable` strategy, the sweet-spot targets are functions with
moderate CRAP, some existing coverage, small enough to understand, and fast
tests. Avoid starting on CRAP-100 god-functions, 0%-coverage code, framework
wiring, and slow integration flows — they produce noisy, hard-to-action
survivors.

## Phase 5 — Generate agent tasks

The strongest brown-field workflow turns survivors into specific work:

```bash
ooze test-mutants \
  --changed-only main \
  --strategy actionable \
  --limit 25 \
  --format agent-tasks-markdown \
  --output ooze-tasks.md \
  --no-fail-on-survivors
```

Hand `ooze-tasks.md` to a developer or an AI agent. Each task names the file,
function, line, the surviving mutation, source context, and CRAP/coverage —
i.e. exactly what's needed to write a targeted test.

## Phase 6 — CI in advisory mode

Start CI non-blocking. Use GitHub annotations:

```yaml
name: ooze

on:
  pull_request:

jobs:
  mutation-advice:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build ooze
        run: cargo build --release

      - name: Run ooze
        run: |
          ./target/release/ooze test-mutants \
            --changed-only origin/main \
            --strategy actionable \
            --limit 25 \
            --jobs 2 \
            --timeout-seconds 120 \
            --format github-annotations \
            --no-fail-on-survivors \
            -- cargo test --jobs 1
```

Prefer SARIF? Swap the format and upload it:

```yaml
      - name: Run ooze (SARIF)
        run: |
          ./target/release/ooze test-mutants \
            --changed-only origin/main \
            --strategy actionable \
            --limit 25 \
            --jobs 2 \
            --timeout-seconds 120 \
            --format sarif \
            --output ooze.sarif \
            --no-fail-on-survivors \
            -- cargo test --jobs 1

      - name: Upload SARIF
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: ooze.sarif
```

Everything after `--` is the probe command. See [`docs/ci.md`](./ci.md) for
more.

## Phase 7 — Soft team policy

Once the output is trusted, agree on a rule (still without failing CI):

> If ooze finds a survived mutant in changed business logic, either add a
> test, mark it intentionally ignored, or explain why the survivor is
> acceptable.

Keep `--no-fail-on-survivors` on and require humans to review the tasks.

## Phase 8 — Gate changed code only

When the team is ready, drop `--no-fail-on-survivors` for changed code so new
work must kill its mutants:

```bash
ooze test-mutants \
  --changed-only origin/main \
  --strategy actionable \
  --limit 50 \
  --format human
```

Keep `allow_incomplete = false` so infrastructure failures (timeouts/errors)
still surface separately from survivors. Do not gate the historical repo yet.

## Phase 9 — Widen scope in slices

Expand module by module, not all at once:

```bash
ooze test-mutants --path crates/domain --strategy actionable --limit 50
# later
ooze test-mutants --path crates/domain --strategy actionable --limit 100
```

## Reference config for brown-field repos

This parses against the current schema (`deny_unknown_fields` rejects unknown
keys, so don't add ones not listed in [`docs/config.md`](./config.md)).

```toml
[scope]
changed_only = "main"
exclude = [
  "dist/**",
  "build/**",
  "coverage/**",
  "**/generated/**",
  "**/*.generated.*",
  "**/fixtures/**",
  "**/testdata/**",
  "**/__snapshots__/**",
  "**/snapshots/**",
]

[mutation]
strategy = "actionable"
limit = 25
static_skips = true
context_lines = 3

[runner]
workspace_backend = "auto"
jobs = 2
timeout_seconds = 120
preflight = true
warmup = true
per_worker_cache = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["cargo", "test", "--jobs", "1"]
env = ["RUST_BACKTRACE=1"]

[report]
format = "agent-tasks-markdown"
output = "ooze-tasks.md"
detail = "compact"
fail_on_survivors = false
allow_incomplete = false
diff = false
stdout = false
stderr = true
only_survivors = true
```

> The `[mutation]` `operators` / `exclude_operators` keys are optional. By
> default every operator implemented for your language runs except
> `integer_zero_one`. Only add an `operators` allow-list if you have a reason
> to narrow it — there are no noisy arithmetic/string/regex operators to
> exclude.

## Tracking adoption

Good signs: survivors lead to real tests; timeouts/errors are rare; the same
functions stop reappearing; developers understand the generated tasks.

Bad signs: lots of equivalent/noisy mutants; frequent timeouts; mutating
fixtures or generated code; oversized reports; ignored annotations.

## Recommended rollout, in one place

1. Local only: `doctor` + `preflight` + `--limit 1`.
2. Local changed-only: `--limit 10`, `agent-tasks-markdown`.
3. PR advisory: GitHub annotations or SARIF, no fail.
4. Team policy: survivors require a test or an explanation.
5. Changed-code gate: drop `--no-fail-on-survivors` for changed files only.
6. Module campaigns: larger manual batches per critical crate/package.

Start tiny, stay on changed code, generate useful tasks, and gate only once
the signal is trusted.
