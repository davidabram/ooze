use std::path::{Path, PathBuf};

use crate::config::{self, OozeConfig};
use crate::core::OperatorName;
use crate::runner::overlay;

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub path: PathBuf,
    pub config_path: Option<PathBuf>,
    pub checks: Vec<CheckResult>,
    pub failed: usize,
    pub warned: usize,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }
}

fn ok(name: &'static str, msg: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        status: CheckStatus::Ok,
        message: msg.into(),
    }
}

fn warn(name: &'static str, msg: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        status: CheckStatus::Warn,
        message: msg.into(),
    }
}

fn fail(name: &'static str, msg: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        status: CheckStatus::Fail,
        message: msg.into(),
    }
}

fn skip(name: &'static str, msg: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        status: CheckStatus::Skip,
        message: msg.into(),
    }
}

fn check_writable(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let probe = dir.join(".ooze-doctor-probe");
    std::fs::write(&probe, b"ok").map_err(|e| format!("write probe: {e}"))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

// Evaluates the [probe].command check: warns when unset/empty, otherwise reports
// whether the binary resolves on PATH.
fn probe_command_check(cmd: Option<&Vec<String>>) -> CheckResult {
    match cmd {
        Some(cmd) if !cmd.is_empty() => {
            let bin = &cmd[0];
            match which(bin) {
                Some(p) => ok(
                    "probe_command",
                    format!("{} found at {}", bin, p.display()),
                ),
                None => warn(
                    "probe_command",
                    format!("{bin} not found on PATH (still ok if invoked via wrapper)"),
                ),
            }
        }
        _ => warn(
            "probe_command",
            "no [probe].command set; pass one after `--` or configure ooze.toml",
        ),
    }
}

// True for a .gitignore content line that contributes a pattern (non-blank and
// not a comment).
fn is_gitignore_pattern(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

fn which(cmd: &str) -> Option<PathBuf> {
    if cmd.contains(std::path::MAIN_SEPARATOR) {
        let p = PathBuf::from(cmd);
        return p.exists().then_some(p);
    }
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn run(path: &Path) -> DoctorReport {
    let mut checks: Vec<CheckResult> = Vec::new();

    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => {
            if p.is_dir() {
                checks.push(ok("repo_root", format!("{} is a directory", p.display())));
                p
            } else {
                checks.push(fail("repo_root", format!("{} is not a directory", p.display())));
                p
            }
        }
        Err(e) => {
            checks.push(fail("repo_root", format!("cannot canonicalize {}: {e}", path.display())));
            path.to_path_buf()
        }
    };

    let cfg_path = canonical.join(config::DEFAULT_CONFIG_NAME);
    let (cfg, cfg_loaded_from) = if cfg_path.exists() {
        match config::load_config(Some(&cfg_path)) {
            Ok((c, loaded)) => {
                checks.push(ok(
                    "ooze_toml",
                    format!("{} parsed cleanly", cfg_path.display()),
                ));
                (c, loaded)
            }
            Err(e) => {
                checks.push(fail("ooze_toml", format!("{}: {e:#}", cfg_path.display())));
                (OozeConfig::default(), None)
            }
        }
    } else {
        checks.push(skip(
            "ooze_toml",
            format!("no {} (defaults will apply)", config::DEFAULT_CONFIG_NAME),
        ));
        (OozeConfig::default(), None)
    };

    checks.push(probe_command_check(cfg.probe.command.as_ref()));

    let requested_backend = cfg.runner.workspace_backend.as_deref().unwrap_or("auto");
    let overlay_ok = overlay::overlay_available();
    match requested_backend {
        "overlay" => {
            if overlay_ok {
                checks.push(ok("workspace_backend", "overlay requested and available"));
            } else {
                checks.push(fail(
                    "workspace_backend",
                    "overlay requested but OverlayFS unavailable (Linux + overlay module needed)",
                ));
            }
        }
        "copy" => checks.push(ok("workspace_backend", "copy backend always available")),
        _ => {
            let resolved = if overlay_ok { "overlay" } else { "copy" };
            checks.push(ok(
                "workspace_backend",
                format!("auto -> {resolved} (overlay_available={overlay_ok})"),
            ));
        }
    }

    let cache_dir = cfg
        .runner
        .cache_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ooze/cache"));
    let cache_abs = if cache_dir.is_absolute() {
        cache_dir
    } else {
        canonical.join(cache_dir)
    };
    match check_writable(&cache_abs) {
        Ok(()) => checks.push(ok("cache_dir", format!("{} writable", cache_abs.display()))),
        Err(e) => checks.push(fail("cache_dir", format!("{}: {e}", cache_abs.display()))),
    }

    let runs_dir = cfg
        .runner
        .runs_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ooze/runs"));
    let runs_abs = if runs_dir.is_absolute() {
        runs_dir
    } else {
        canonical.join(runs_dir)
    };
    match check_writable(&runs_abs) {
        Ok(()) => checks.push(ok("runs_dir", format!("{} writable", runs_abs.display()))),
        Err(e) => checks.push(fail("runs_dir", format!("{}: {e}", runs_abs.display()))),
    }

    match cfg.mutation.lcov.as_ref() {
        Some(p) => {
            let resolved = if p.is_absolute() { p.clone() } else { canonical.join(p) };
            if resolved.is_file() {
                checks.push(ok("lcov", format!("{} exists", resolved.display())));
            } else {
                checks.push(fail("lcov", format!("{} not found", resolved.display())));
            }
        }
        None => checks.push(skip("lcov", "no [mutation].lcov configured")),
    }

    let gitignore = canonical.join(".gitignore");
    let mut excludes_msg = format!(
        "defaults={}, user={}",
        crate::DEFAULT_EXCLUDES.len(),
        cfg.scope.exclude.len()
    );
    if gitignore.is_file() {
        let lines = std::fs::read_to_string(&gitignore)
            .map_or(0, |s| s.lines().filter(|l| is_gitignore_pattern(l)).count());
        excludes_msg = format!("{excludes_msg}, gitignore={lines}");
        checks.push(ok("excludes", excludes_msg));
    } else {
        checks.push(warn("excludes", format!("{excludes_msg}, no .gitignore at repo root")));
    }

    let env_target = std::env::var("CARGO_TARGET_DIR").ok();
    let cfg_build_cache = cfg.runner.build_cache_dir.clone();
    match (env_target, cfg_build_cache) {
        (Some(e), Some(c)) => checks.push(warn(
            "build_cache_dir",
            format!(
                "CARGO_TARGET_DIR={e} but [runner].build_cache_dir={} also set; CLI/config wins for probes",
                c.display()
            ),
        )),
        (Some(e), None) => checks.push(ok(
            "build_cache_dir",
            format!("CARGO_TARGET_DIR={e} (inherited)"),
        )),
        (None, Some(c)) => {
            let resolved = if c.is_absolute() { c.clone() } else { canonical.join(&c) };
            checks.push(ok(
                "build_cache_dir",
                format!("configured -> {}", resolved.display()),
            ));
        }
        (None, None) => checks.push(skip(
            "build_cache_dir",
            "unset; default will be derived from cache_dir",
        )),
    }

    if let Some(ops) = cfg.mutation.operators.as_ref() {
        let unknown: Vec<&String> = ops
            .iter()
            .filter(|o| OperatorName::parse(o).is_none())
            .collect();
        if unknown.is_empty() {
            checks.push(ok(
                "operators",
                format!("{} operator(s) configured, all known", ops.len()),
            ));
        } else {
            checks.push(fail(
                "operators",
                format!("unknown operator(s) in [mutation].operators: {unknown:?}"),
            ));
        }
    }

    let failed = checks
        .iter()
        .filter(|c| matches!(c.status, CheckStatus::Fail))
        .count();
    let warned = checks
        .iter()
        .filter(|c| matches!(c.status, CheckStatus::Warn))
        .count();

    DoctorReport {
        path: canonical,
        config_path: cfg_loaded_from,
        checks,
        failed,
        warned,
    }
}

pub fn print_human(report: &DoctorReport) {
    println!("ooze doctor: {}", report.path.display());
    if let Some(p) = &report.config_path {
        println!("  config: {}", p.display());
    } else {
        println!("  config: (none)");
    }
    for c in &report.checks {
        let tag = match c.status {
            CheckStatus::Ok => "[ ok ]",
            CheckStatus::Warn => "[warn]",
            CheckStatus::Fail => "[FAIL]",
            CheckStatus::Skip => "[skip]",
        };
        println!("  {tag} {:<18} {}", c.name, c.message);
    }
    println!(
        "summary: {} failed, {} warnings, {} total",
        report.failed,
        report.warned,
        report.checks.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_command_check_warns_when_unset() {
        let c = probe_command_check(None);
        assert!(matches!(c.status, CheckStatus::Warn));
        assert!(c.message.contains("no [probe].command set"));
    }

    #[test]
    fn probe_command_check_reports_missing_binary() {
        // A non-empty command takes the resolve branch; a path that doesn't
        // exist resolves to "not found", distinct from the unset warning.
        let c = probe_command_check(Some(&vec![
            "/no/such/binary/xyzzy".to_string(),
        ]));
        assert!(matches!(c.status, CheckStatus::Warn));
        assert!(c.message.contains("not found on PATH"));
    }

    #[test]
    fn probe_command_check_ok_when_binary_resolves() {
        let exe = std::env::current_exe().expect("current exe");
        let c = probe_command_check(Some(&vec![exe.to_string_lossy().into_owned()]));
        assert!(matches!(c.status, CheckStatus::Ok));
        assert!(c.message.contains("found at"));
    }

    #[test]
    fn is_gitignore_pattern_rejects_blank_and_comments() {
        assert!(!is_gitignore_pattern(""));
        assert!(!is_gitignore_pattern("   "));
        assert!(!is_gitignore_pattern("# comment"));
        assert!(!is_gitignore_pattern("   # indented comment"));
    }

    #[test]
    fn is_gitignore_pattern_accepts_real_patterns() {
        assert!(is_gitignore_pattern("*.log"));
        assert!(is_gitignore_pattern("  target/  "));
    }

    #[test]
    fn run_operators_check_ok_when_all_known() {
        // All configured operators parse, so `unknown` is empty and the check is
        // Ok. Distinguishes `is_none`/`is_some` (line 278) and
        // `is_empty`/`!is_empty` (line 280): either mutation flips this to Fail.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join(config::DEFAULT_CONFIG_NAME),
            "[mutation]\noperators = [\"negate_equality\", \"comparison_boundary\"]\n",
        )
        .expect("write config");

        let report = run(dir.path());
        let op = report
            .checks
            .iter()
            .find(|c| c.name == "operators")
            .expect("operators check present");
        assert!(
            matches!(op.status, CheckStatus::Ok),
            "all-known operators should be Ok, got {:?}: {}",
            op.status,
            op.message
        );
        assert!(op.message.contains("all known"));
    }

    #[test]
    fn run_operators_check_fails_on_unknown() {
        // An unconfigured operator name makes `unknown` non-empty, so the check
        // is Fail. Kills the `is_empty -> !is_empty` mutation (line 280) in the
        // opposite direction from the all-known case.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join(config::DEFAULT_CONFIG_NAME),
            "[mutation]\noperators = [\"negate_equality\", \"definitely_not_an_operator\"]\n",
        )
        .expect("write config");

        let report = run(dir.path());
        let op = report
            .checks
            .iter()
            .find(|c| c.name == "operators")
            .expect("operators check present");
        assert!(
            matches!(op.status, CheckStatus::Fail),
            "unknown operator should Fail, got {:?}: {}",
            op.status,
            op.message
        );
        assert!(op.message.contains("unknown operator"));
    }
}
