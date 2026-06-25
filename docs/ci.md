# Running ooze in CI

Ooze is built for CI from the start: structured output, deterministic exit
codes, and per-survivor annotations.

## Exit codes

| Code | Meaning                                                           |
| ---- | ----------------------------------------------------------------- |
| 0    | Run completed; no survived mutants, no timeouts/errors.           |
| 1    | Run completed; survived mutants found.                            |
| 2    | Preflight failed or timed out — probe is broken on clean code.    |
| 3    | Run completed but timeout/error outcomes occurred (incomplete).   |
| 4    | Usage / invalid invocation (reserved).                            |
| 5    | Internal ooze error (reserved).                                   |

Priority order when multiple apply: preflight > infrastructure > survivors > success.

Overrides:
- `--no-fail-on-survivors` — exit 0 even if survivors are found (timeouts/errors still surface).
- `--allow-incomplete` — treat timeout/error outcomes as non-fatal.

## GitHub Actions

### Rollout phase: annotate without failing

Use this while you're still tuning the tool. PRs get inline annotations on
survived mutants, but the job stays green.

```yaml
- name: Build ooze
  run: cargo build --release --bin ooze

- name: Mutation testing
  run: |
    ./target/release/ooze test-mutants \
      --path . \
      --preflight \
      --strategy actionable \
      --limit 50 \
      --jobs 2 \
      --timeout-seconds 120 \
      --per-worker-cache \
      --warmup \
      --probe-env CARGO_TARGET_DIR={build_cache} \
      --format github-annotations \
      --no-fail-on-survivors \
      -- cargo test --jobs 1
```

Each survived mutant becomes a `::warning file=…,line=…,title=Ooze survived
mutant::…` line. GitHub renders them inline in the PR's "Files changed" tab.

### Enforcement phase: fail PRs on new survivors

Once you trust the signal, drop `--no-fail-on-survivors`. Survivors → exit 1 →
red check.

```yaml
- name: Mutation testing (gating)
  run: |
    ./target/release/ooze test-mutants \
      --path . \
      --preflight \
      --strategy actionable \
      --limit 50 \
      --jobs 2 \
      --timeout-seconds 120 \
      --per-worker-cache \
      --warmup \
      --probe-env CARGO_TARGET_DIR={build_cache} \
      --format github-annotations \
      -- cargo test --jobs 1
```

### Caching the build cache dir

Per-worker build cache dirs live under `.ooze/cache/build-cache-job-{0..jobs-1}`.
Cache them between runs to turn cold builds into warm cache hits:

```yaml
- uses: actions/cache@v4
  with:
    path: .ooze/cache
    key: ooze-cache-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: ooze-cache-${{ runner.os }}-
```

Combined with `--warmup`, this turns the "first mutant per worker is a cold
compile" cost into a cache hit on most runs.

### Uploading the JSON report as an artifact

Run twice (or capture stderr separately) if you want both annotations and a
machine-readable report:

```yaml
- name: Mutation report
  run: |
    ./target/release/ooze test-mutants \
      --path . --preflight --strategy actionable --limit 50 \
      --jobs 2 --timeout-seconds 120 --per-worker-cache --warmup \
      --probe-env CARGO_TARGET_DIR={build_cache} \
      --format json --no-fail-on-survivors \
      -- cargo test --jobs 1 > ooze-report.json

- uses: actions/upload-artifact@v4
  with:
    name: ooze-report
    path: ooze-report.json
```

### Comment a task list on the PR

`--format agent-tasks-markdown` emits a copy-pasteable test-writing task list:

```yaml
- name: Mutation task list
  run: |
    ./target/release/ooze test-mutants \
      --path . --preflight --strategy actionable --limit 30 \
      --jobs 2 --timeout-seconds 120 --per-worker-cache --warmup \
      --probe-env CARGO_TARGET_DIR={build_cache} \
      --format agent-tasks-markdown --no-fail-on-survivors \
      -- cargo test --jobs 1 > ooze-tasks.md

- name: Post task list
  if: github.event_name == 'pull_request'
  run: gh pr comment ${{ github.event.pull_request.number }} --body-file ooze-tasks.md
  env:
    GH_TOKEN: ${{ github.token }}
```

### SARIF for GitHub Code Scanning

`--format sarif` emits a SARIF 2.1.0 log. Upload it with the CodeQL action and
survived mutants show up in the repo's **Security → Code scanning** tab,
deduplicated across runs and persisted beyond the PR.

```yaml
permissions:
  security-events: write
  contents: read

steps:
  - name: Build ooze
    run: cargo build --release --bin ooze

  - name: Mutation testing (SARIF)
    run: |
      ./target/release/ooze test-mutants \
        --path . \
        --preflight \
        --strategy actionable \
        --limit 50 \
        --jobs 2 \
        --timeout-seconds 120 \
        --per-worker-cache \
        --warmup \
        --probe-env CARGO_TARGET_DIR={build_cache} \
        --format sarif \
        --output ooze.sarif \
        --no-fail-on-survivors \
        -- cargo test --jobs 1

  - name: Upload SARIF
    uses: github/codeql-action/upload-sarif@v3
    if: always()
    with:
      sarif_file: ooze.sarif
      category: ooze-mutants
```

What's in the SARIF:

- `tool.driver.name = "ooze"`.
- **One rule per operator** that produced a survivor (e.g.
  `ooze.survived_mutant.comparison_boundary`). The rule's `fullDescription` is the
  operator description; `helpUri` and `help.text` point at the operator's test
  hint.
- **One result per survived mutant.** `level: warning`,
  `message.text` is the same test-writing prompt used by
  `agent-tasks-markdown`, and `locations[0]` carries the file URI (relative,
  no leading `./`), 1-based `startLine`, and 1-based `startColumn`.
- Killed / timeout / error outcomes are **not** emitted — SARIF is a survivors
  report, not a run log.

SARIF vs `github-annotations`:

| Format               | Where it shows up                   | Lifetime          |
| -------------------- | ----------------------------------- | ----------------- |
| `github-annotations` | PR "Files changed" tab, inline      | Tied to the run   |
| `sarif`              | Security → Code scanning, per-alert | Persists, dedupes |

You can emit both in the same job — run once with `--format github-annotations`
for PR review and once with `--format sarif --output ooze.sarif` for upload.
Combine with `--no-fail-on-survivors` during rollout; drop it to gate.

## GitLab CI / generic CI

Other CI systems don't render `github-annotations`, so use JSON and let the
exit code drive pass/fail:

```yaml
mutation_testing:
  script:
    - cargo build --release --bin ooze
    - ./target/release/ooze test-mutants
        --path . --preflight --strategy actionable --limit 50
        --jobs 2 --timeout-seconds 120 --per-worker-cache --warmup
        --probe-env CARGO_TARGET_DIR={build_cache}
        --format json
        -- cargo test --jobs 1 | tee ooze-report.json
  artifacts:
    paths:
      - ooze-report.json
    when: always
  cache:
    key: "ooze-$CI_COMMIT_REF_SLUG"
    paths:
      - .ooze/cache
```

## Recommended flags for any CI

- `--preflight` — fail fast if the probe is broken on clean code.
- `--strategy actionable` — run the most informative mutants first.
- `--limit N` — bound wall time. Tune so the job finishes in a budget you'd accept on every PR.
- `--timeout-seconds T` — kill stuck probes. Set to ~2x your slowest test.
- `--jobs J` + `--per-worker-cache` + `--warmup` — parallelism without build lock contention and without cold-start cost.
- `--coverage <spec>` — if you already collect coverage, feed it in; the scheduler ranks better and CRAP scores become meaningful. Pass `format:path` (`lcov`, `cobertura`, `jacoco`, `go-cover`) or a bare path to auto-detect, e.g. `--coverage cobertura:coverage.xml` or `--coverage coverage.out`. Repeatable, and reports are merged — pass one per suite in a monorepo (`--coverage lcov:frontend/lcov.info --coverage jacoco:backend/jacoco.xml`). ooze prints match diagnostics (matched/unmatched source files) to stderr so path-root mismatches are visible. (`--lcov lcov.info` still works as a deprecated alias.)

## Tuning loop

1. Start with `--limit 30 --no-fail-on-survivors --format json` on a feature branch. Inspect the report.
2. Switch to `--format github-annotations --no-fail-on-survivors` on PRs.
3. Address survivors using the prompts in `--format agent-tasks-markdown`.
4. When the noise floor is acceptable, drop `--no-fail-on-survivors` to enforce.
