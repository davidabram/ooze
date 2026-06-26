use std::process::Command;

fn ooze() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ooze"))
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

// ── scan ──────────────────────────────────────────────────────────────────────

#[test]
fn scan_json_outputs_valid_json() {
    let out = ooze()
        .args(["scan", "--path", "tests/fixtures/lang", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout should be valid JSON when --format json");
}

#[test]
fn scan_non_json_produces_no_output() {
    let out = ooze()
        .args(["scan", "--path", "tests/fixtures/lang", "--format", "human"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "expected no stdout for non-json format, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ── mutants ───────────────────────────────────────────────────────────────────

#[test]
fn mutants_json_outputs_valid_json() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/mutate", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout should be valid JSON when --format json");
}

#[test]
fn mutants_non_json_produces_no_output() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/mutate", "--format", "human"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "expected no stdout for non-json format, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ── operators ─────────────────────────────────────────────────────────────────

#[test]
fn operators_json_outputs_valid_json() {
    let out = ooze()
        .args(["operators", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout should be valid JSON when --format json");
}

#[test]
fn operators_non_json_outputs_text() {
    let out = ooze()
        .args(["operators", "--format", "human"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.is_empty(), "expected text output for non-json format");
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "non-json format should not produce JSON"
    );
}

// ── plan-mutants ──────────────────────────────────────────────────────────────

#[test]
fn plan_mutants_json_outputs_valid_json() {
    let out = ooze()
        .args(["plan-mutants", "--path", "tests/fixtures/mutate", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout should be valid JSON when --format json");
}

#[test]
fn plan_mutants_non_json_produces_no_output() {
    let out = ooze()
        .args(["plan-mutants", "--path", "tests/fixtures/mutate", "--format", "human"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "expected no stdout for non-json format, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ── test-mutant (singular) ────────────────────────────────────────────────────

#[test]
fn test_mutant_fails_with_unknown_id() {
    let out = ooze()
        .args(["test-mutant", "--path", "tests/fixtures/mutate", "--id", "nonexistent-id", "--", "echo", "ok"])
        .output()
        .expect("failed to run ooze");
    assert!(!out.status.success(), "expected failure for unknown mutation id");
}

#[test]
fn test_mutant_succeeds_with_valid_id() {
    // Use the absolute path so that scan_directory produces absolute file paths and
    // test-mutant's canonicalize call produces a matching prefix for strip_prefix.
    let fixture = std::fs::canonicalize("tests/fixtures/mutate").unwrap();
    let fixture_str = fixture.to_str().unwrap();

    let mutants_out = ooze()
        .args(["mutants", "--path", fixture_str, "--format", "json"])
        .output()
        .expect("failed to run mutants");
    assert!(mutants_out.status.success());
    let candidates: Vec<serde_json::Value> =
        serde_json::from_slice(&mutants_out.stdout).expect("mutants output should be JSON");
    assert!(!candidates.is_empty(), "expected at least one mutation candidate");
    let id = candidates[0]["id"].as_str().expect("candidate should have an id field");

    let out = ooze()
        .args(["test-mutant", "--path", fixture_str, "--id", id, "--", "echo", "ok"])
        .output()
        .expect("failed to run test-mutant");
    assert!(
        out.status.success(),
        "expected success for known id {id}; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("test-mutant should output JSON");
    assert_eq!(
        result["candidate"]["id"].as_str(),
        Some(id),
        "test-mutant should apply the candidate whose id matches the requested id"
    );
}

// ── test-mutants preflight format ─────────────────────────────────────────────

#[test]
fn test_mutants_preflight_failure_json_prints_to_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let out = ooze()
        .args([
            "test-mutants",
            "--path", "tests/fixtures/mutate",
            "--preflight",
            "--format", "json",
            "--limit", "0",
            "--cache-dir", tmp.path().join("cache").to_str().unwrap(),
            "--runs-dir", tmp.path().join("runs").to_str().unwrap(),
            "--",
            "false",
        ])
        .output()
        .expect("failed to run ooze");
    assert!(!out.status.success(), "preflight failure should exit non-zero");
    serde_json::from_slice::<serde_json::Value>(&out.stdout)
        .expect("preflight failure with --format json should print JSON to stdout");
}

#[test]
fn test_mutants_preflight_failure_human_prints_to_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let out = ooze()
        .args([
            "test-mutants",
            "--path", "tests/fixtures/mutate",
            "--preflight",
            "--format", "human",
            "--limit", "0",
            "--cache-dir", tmp.path().join("cache").to_str().unwrap(),
            "--runs-dir", tmp.path().join("runs").to_str().unwrap(),
            "--",
            "false",
        ])
        .output()
        .expect("failed to run ooze");
    assert!(!out.status.success(), "preflight failure should exit non-zero");
    assert!(
        out.stdout.is_empty(),
        "preflight failure with --format human should not print to stdout"
    );
    assert!(
        !out.stderr.is_empty(),
        "preflight failure with --format human should print to stderr"
    );
}

// ── apply-mutant ──────────────────────────────────────────────────────────────

#[test]
fn apply_mutant_fails_with_unknown_id() {
    let out = ooze()
        .args(["apply-mutant", "--path", "tests/fixtures/mutate", "--id", "nonexistent-id"])
        .output()
        .expect("failed to run ooze");
    assert!(!out.status.success(), "expected failure for unknown mutation id");
}

#[test]
fn apply_mutant_succeeds_with_valid_id() {
    // Get a real candidate ID from the fixture.
    let mutants_out = ooze()
        .args(["mutants", "--path", "tests/fixtures/mutate", "--format", "json"])
        .output()
        .expect("failed to run mutants");
    assert!(mutants_out.status.success());
    let candidates: Vec<serde_json::Value> =
        serde_json::from_slice(&mutants_out.stdout).expect("mutants output should be JSON");
    assert!(!candidates.is_empty(), "expected at least one mutation candidate");
    let id = candidates[0]["id"].as_str().expect("candidate should have an id field");

    // Apply that specific mutation and verify we get a diff.
    let out = ooze()
        .args(["apply-mutant", "--path", "tests/fixtures/mutate", "--id", id])
        .output()
        .expect("failed to run apply-mutant");
    assert!(
        out.status.success(),
        "expected success for known id {id}; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.stdout.is_empty(),
        "expected a non-empty diff for id {id}"
    );
}

// ── init-config ───────────────────────────────────────────────────────────────

#[test]
fn init_config_creates_file_when_missing() {
    let tmp = tempdir();
    let cfg = tmp.path().join("ooze.toml");
    let out = ooze()
        .args(["init-config", "--path", cfg.to_str().unwrap(), "--language", "rust"])
        .output()
        .expect("failed to run ooze");
    assert!(
        out.status.success(),
        "should succeed when file does not exist; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(cfg.exists(), "config file should have been written");
}

#[test]
fn init_config_fails_when_file_exists_without_force() {
    let tmp = tempdir();
    let cfg = tmp.path().join("ooze.toml");
    std::fs::write(&cfg, "existing").unwrap();
    let out = ooze()
        .args(["init-config", "--path", cfg.to_str().unwrap(), "--language", "rust"])
        .output()
        .expect("failed to run ooze");
    assert!(!out.status.success(), "should fail when file exists and --force is not set");
}

#[test]
fn init_config_overwrites_with_force() {
    let tmp = tempdir();
    let cfg = tmp.path().join("ooze.toml");
    std::fs::write(&cfg, "old-content").unwrap();
    let out = ooze()
        .args(["init-config", "--path", cfg.to_str().unwrap(), "--language", "rust", "--force"])
        .output()
        .expect("failed to run ooze");
    assert!(
        out.status.success(),
        "should succeed with --force even when file exists; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let content = std::fs::read_to_string(&cfg).unwrap();
    assert_ne!(content, "old-content", "file should have been overwritten");
}

// ── operator fixture snapshot ─────────────────────────────────────────────────

/// Project a discovered mutant down to the fields that are stable across
/// refactors: language, operator, implementation, function, original,
/// replacement, and line. Everything path- or offset-dependent (`id`, `file`,
/// `start_byte`/`end_byte`, `column`) is dropped so the snapshot only breaks
/// when an operator's *behaviour* changes, not when the fixture moves on disk.
fn stable_fields(c: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "language": c["language"],
        "operator": c["operator"],
        "implementation": c["implementation"],
        "function": c["function"],
        "original": c["original"],
        "replacement": c["replacement"],
        "line": c["line"],
    })
}

/// Sort key that is total over the stable projection, so two runs (and the
/// golden file) compare order-independently.
fn snapshot_sorted(mut mutants: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    mutants.sort_by_key(|c| c.to_string());
    mutants
}

#[test]
fn rust_operator_fixture_matches_snapshot() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/operators/rust/all.rs", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let discovered: Vec<serde_json::Value> =
        serde_json::from_slice(&out.stdout).expect("mutants output should be JSON");
    let got = snapshot_sorted(discovered.iter().map(stable_fields).collect());

    let expected_raw = std::fs::read_to_string("tests/fixtures/operators/rust/expected.json")
        .expect("expected.json fixture should exist");
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&expected_raw).expect("expected.json should be valid JSON");
    let want = snapshot_sorted(expected);

    assert_eq!(
        got, want,
        "discovered Rust mutants drifted from tests/fixtures/operators/rust/expected.json"
    );

    // Guard the headline promise: every one of the 23 Rust operators still fires.
    let operators: std::collections::BTreeSet<&str> = discovered
        .iter()
        .map(|c| c["operator"].as_str().expect("operator should be a string"))
        .collect();
    assert_eq!(
        operators.len(),
        23,
        "expected all 23 Rust operators to fire, got: {operators:?}"
    );
}

#[test]
fn python_operator_fixture_matches_snapshot() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/operators/python/all.py", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let discovered: Vec<serde_json::Value> =
        serde_json::from_slice(&out.stdout).expect("mutants output should be JSON");
    let got = snapshot_sorted(discovered.iter().map(stable_fields).collect());

    let expected_raw = std::fs::read_to_string("tests/fixtures/operators/python/expected.json")
        .expect("expected.json fixture should exist");
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&expected_raw).expect("expected.json should be valid JSON");
    let want = snapshot_sorted(expected);

    assert_eq!(
        got, want,
        "discovered Python mutants drifted from tests/fixtures/operators/python/expected.json"
    );

    // Guard the headline promise: every one of the 20 Python operators still fires.
    let operators: std::collections::BTreeSet<&str> = discovered
        .iter()
        .map(|c| c["operator"].as_str().expect("operator should be a string"))
        .collect();
    assert_eq!(
        operators.len(),
        20,
        "expected all 20 Python operators to fire, got: {operators:?}"
    );
}

#[test]
fn javascript_operator_fixture_matches_snapshot() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/operators/javascript/all.js", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let discovered: Vec<serde_json::Value> =
        serde_json::from_slice(&out.stdout).expect("mutants output should be JSON");
    let got = snapshot_sorted(discovered.iter().map(stable_fields).collect());

    let expected_raw = std::fs::read_to_string("tests/fixtures/operators/javascript/expected.json")
        .expect("expected.json fixture should exist");
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&expected_raw).expect("expected.json should be valid JSON");
    let want = snapshot_sorted(expected);

    assert_eq!(
        got, want,
        "discovered JavaScript mutants drifted from tests/fixtures/operators/javascript/expected.json"
    );

    // Guard the headline promise: every one of the 17 JavaScript operators still fires.
    let operators: std::collections::BTreeSet<&str> = discovered
        .iter()
        .map(|c| c["operator"].as_str().expect("operator should be a string"))
        .collect();
    assert_eq!(
        operators.len(),
        17,
        "expected all 17 JavaScript operators to fire, got: {operators:?}"
    );
}

#[test]
fn typescript_operator_fixture_matches_snapshot() {
    let out = ooze()
        .args(["mutants", "--path", "tests/fixtures/operators/typescript/all.ts", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let discovered: Vec<serde_json::Value> =
        serde_json::from_slice(&out.stdout).expect("mutants output should be JSON");
    let got = snapshot_sorted(discovered.iter().map(stable_fields).collect());

    let expected_raw = std::fs::read_to_string("tests/fixtures/operators/typescript/expected.json")
        .expect("expected.json fixture should exist");
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&expected_raw).expect("expected.json should be valid JSON");
    let want = snapshot_sorted(expected);

    assert_eq!(
        got, want,
        "discovered TypeScript mutants drifted from tests/fixtures/operators/typescript/expected.json"
    );

    // Guard the headline promise: every one of the 17 TypeScript operators still fires.
    let operators: std::collections::BTreeSet<&str> = discovered
        .iter()
        .map(|c| c["operator"].as_str().expect("operator should be a string"))
        .collect();
    assert_eq!(
        operators.len(),
        17,
        "expected all 17 TypeScript operators to fire, got: {operators:?}"
    );
}

// ── crap ──────────────────────────────────────────────────────────────────────

#[test]
fn crap_json_format_outputs_valid_json() {
    let out = ooze()
        .args(["crap", "--path", "tests/fixtures/lang", "--format", "json"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.is_empty(), "expected JSON output for --format json");
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout should be valid JSON when --format json");
}

#[test]
fn crap_non_json_format_produces_no_stdout() {
    let out = ooze()
        .args(["crap", "--path", "tests/fixtures/lang", "--format", "human"])
        .output()
        .expect("failed to run ooze");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "expected no stdout for non-json format, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
