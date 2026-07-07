//! Resolution of `test-mutants` arguments into a fully-typed, runnable settings
//! struct. This is where CLI flags, the loaded `ooze.toml`, and built-in defaults
//! are merged exactly once, before any domain logic runs. The command body in
//! `super::run` then consumes [`ResolvedTestMutants`] without re-deriving anything.

use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{PackageManager, Preset, TestMutantsArgs, WorkspaceBackendArg};
use crate::{config, mutate, report, runner, scheduler};

/// Everything `test-mutants` needs to run, with every CLI/config/default decision
/// already made. Notably `probe` is non-empty by construction, so the command
/// body can't observe the "no probe command" invalid state.
pub(crate) struct ResolvedTestMutants {
    pub(crate) path: PathBuf,
    pub(crate) strategy: scheduler::MutationStrategy,
    pub(crate) limit: Option<usize>,
    pub(crate) jobs: usize,
    pub(crate) timeout: Option<Duration>,
    pub(crate) build_cache_dir: Option<PathBuf>,
    pub(crate) per_worker_cache: bool,
    pub(crate) warmup: bool,
    pub(crate) workspace_backend: WorkspaceBackendArg,
    pub(crate) cache_dir: PathBuf,
    pub(crate) runs_dir: PathBuf,
    pub(crate) format: report::ReportFormat,
    pub(crate) output: Option<PathBuf>,
    pub(crate) report_opts: report::ReportOptions,
    pub(crate) no_static_skips: bool,
    pub(crate) context_lines: usize,
    pub(crate) preflight: bool,
    pub(crate) no_fail_on_survivors: bool,
    pub(crate) allow_incomplete: bool,
    pub(crate) coverage: Vec<String>,
    pub(crate) lcov: Option<PathBuf>,
    pub(crate) excludes: Vec<String>,
    pub(crate) filter: mutate::OperatorFilter,
    pub(crate) probe_env: Vec<runner::ProbeEnvTemplate>,
    pub(crate) probe: Vec<String>,
    pub(crate) changed_only: Option<String>,
    pub(crate) progress_enabled: bool,
    pub(crate) pre_run: Option<Vec<String>>,
}

/// Merge CLI args, the resolved `ooze.toml`, and defaults into a runnable struct.
pub(crate) fn test_mutants(args: TestMutantsArgs) -> anyhow::Result<ResolvedTestMutants> {
    use super::{
        build_report_options, parse_report_detail_str, parse_report_format_str,
        parse_strategy_str, parse_workspace_backend_str, progress_enabled, resolve_bool_flag,
        resolve_disabled_flag, resolve_exclude_list, resolve_exclude_operators, resolve_excludes,
        resolve_operators, resolve_probe_env,
    };

    let TestMutantsArgs {
        config: config_path,
        path,
        lcov,
        coverage,
        strategy,
        limit,
        jobs,
        timeout_seconds,
        build_cache_dir,
        per_worker_cache,
        warmup,
        workspace_backend,
        preset,
        cache_dir,
        runs_dir,
        format,
        output,
        report_detail,
        no_diff,
        no_stdout,
        no_stderr,
        only_survivors,
        exclude,
        changed_only,
        probe_env,
        operators,
        exclude_operators,
        no_static_skips,
        context_lines,
        preflight,
        no_fail_on_survivors,
        allow_incomplete,
        quiet,
        progress,
        probe,
    } = args;

    let (cfg, cfg_loaded_from) = config::load_config(config_path.as_deref(), &path)?;
    if let Some(p) = &cfg_loaded_from {
        eprintln!("ooze: loaded config from {}", p.display());
    }

    if let Some(p) = preset
        && !path.join(p.marker_file()).exists()
    {
        anyhow::bail!(
            "{} preset requires a {} at the project path ({})",
            p.name(),
            p.marker_file(),
            path.display()
        );
    }
    // Human-readable record of every default the preset filled, printed once
    // below so preset expansion stays debuggable.
    let mut preset_fills: Vec<String> = Vec::new();

    let strategy = match strategy {
        Some(s) => s,
        None => match cfg.mutation.strategy.as_deref() {
            Some(s) => parse_strategy_str(s)?,
            None => scheduler::MutationStrategy::Discovery,
        },
    };
    let limit = limit.or(cfg.mutation.limit);
    let jobs = jobs.or(cfg.runner.jobs).unwrap_or(1);
    let timeout = timeout_seconds
        .or(cfg.runner.timeout_seconds)
        .map(Duration::from_secs);
    let build_cache_dir = build_cache_dir.or(cfg.runner.build_cache_dir.clone());
    // Only the rust preset turns on per-worker caches: Cargo target dirs fight
    // over locks when shared, while Go's build cache is designed to be shared.
    let preset_per_worker_cache = preset == Some(Preset::Rust);
    let per_worker_cache = if !per_worker_cache && cfg.runner.per_worker_cache.is_none() && preset_per_worker_cache {
        preset_fills.push("per_worker_cache=true".into());
        true
    } else {
        resolve_bool_flag(per_worker_cache, cfg.runner.per_worker_cache)
    };
    let warmup = if !warmup && cfg.runner.warmup.is_none() && preset.is_some() {
        preset_fills.push("warmup=true".into());
        true
    } else {
        resolve_bool_flag(warmup, cfg.runner.warmup)
    };
    let workspace_backend = match workspace_backend {
        Some(w) => w,
        None => match cfg.runner.workspace_backend.as_deref() {
            Some(s) => parse_workspace_backend_str(s)?,
            None if preset.is_some() => {
                preset_fills.push("workspace_backend=worktree".into());
                WorkspaceBackendArg::Worktree
            }
            None => WorkspaceBackendArg::Auto,
        },
    };
    let cache_dir = cache_dir
        .or(cfg.runner.cache_dir.clone())
        .unwrap_or_else(|| PathBuf::from(".ooze/cache"));
    let runs_dir = runs_dir
        .or(cfg.runner.runs_dir.clone())
        .unwrap_or_else(|| PathBuf::from(".ooze/runs"));
    let format = match format {
        Some(f) => f,
        None => match cfg.report.format.as_deref() {
            Some(s) => parse_report_format_str(s)?,
            None => report::ReportFormat::Json,
        },
    };
    let output = output.or(cfg.report.output.clone());

    let report_detail = match report_detail {
        Some(d) => d,
        None => match cfg.report.detail.as_deref() {
            Some(s) => parse_report_detail_str(s)?,
            None => format.default_detail(),
        },
    };
    let report_opts = build_report_options(
        report_detail,
        no_diff,
        cfg.report.diff,
        no_stdout,
        cfg.report.stdout,
        no_stderr,
        cfg.report.stderr,
        only_survivors,
        cfg.report.only_survivors,
    );

    let no_static_skips = resolve_disabled_flag(no_static_skips, cfg.mutation.static_skips);
    let context_lines = context_lines.or(cfg.mutation.context_lines).unwrap_or(3);
    let preflight = resolve_bool_flag(preflight, cfg.runner.preflight);
    let no_fail_on_survivors =
        resolve_disabled_flag(no_fail_on_survivors, cfg.report.fail_on_survivors);
    let allow_incomplete = resolve_bool_flag(allow_incomplete, cfg.report.allow_incomplete);
    let lcov = lcov.or(cfg.mutation.lcov.clone());
    let coverage = if coverage.is_empty() {
        cfg.mutation.coverage.clone()
    } else {
        coverage
    };

    let exclude = resolve_exclude_list(exclude, &cfg.scope.exclude);
    let excludes = resolve_excludes(&path, &exclude);

    let mut probe_env: Vec<runner::ProbeEnvTemplate> = resolve_probe_env(probe_env, &cfg.probe.env)?
        .into_iter()
        .map(|(k, v)| runner::ProbeEnvTemplate::parse(k, &v))
        .collect();
    match preset {
        Some(Preset::Rust) => preset_fills.extend(rust_preset_probe_env_fills(&mut probe_env)),
        Some(Preset::Go) => preset_fills.extend(go_preset_probe_env_fills(&mut probe_env)),
        Some(Preset::Node) => preset_fills.extend(node_preset_probe_env_fills(
            &mut probe_env,
            PackageManager::detect(&path),
        )),
        None => {}
    }

    let operators = resolve_operators(
        operators,
        cfg.mutation.operators.as_ref(),
        cfg.mutation.categories.as_ref(),
    )?;
    let exclude_operators = resolve_exclude_operators(
        exclude_operators,
        &cfg.mutation.exclude_operators,
        &cfg.mutation.exclude_categories,
    )?;
    let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);

    let changed_only = changed_only.or(cfg.scope.changed_only.clone());

    let progress_enabled = progress_enabled(quiet, progress.resolve());

    let mut probe = probe;
    if probe.is_empty() {
        if let Some(cmd) = cfg.probe.command.as_ref() {
            probe.clone_from(cmd);
        } else if let Some(p) = preset {
            let cmd = default_probe(p, &path);
            preset_fills.push(format!("probe=`{}`", cmd.join(" ")));
            probe = cmd;
        } else {
            anyhow::bail!(
                "missing probe command; pass one after `--` or set [probe].command in ooze.toml"
            );
        }
    }

    if let Some(p) = preset
        && !preset_fills.is_empty()
    {
        eprintln!("ooze: preset {}: {}", p.name(), preset_fills.join(", "));
    }

    let pre_run = match cfg.runner.pre_run.clone() {
        Some(cmd) if cmd.is_empty() => {
            anyhow::bail!("[runner].pre_run must not be an empty command")
        }
        other => other,
    };

    Ok(ResolvedTestMutants {
        path,
        strategy,
        limit,
        jobs,
        timeout,
        build_cache_dir,
        per_worker_cache,
        warmup,
        workspace_backend,
        cache_dir,
        runs_dir,
        format,
        output,
        report_opts,
        no_static_skips,
        context_lines,
        preflight,
        no_fail_on_survivors,
        allow_incomplete,
        coverage,
        lcov,
        excludes,
        filter,
        probe_env,
        probe,
        changed_only,
        progress_enabled,
        pre_run,
    })
}

/// The probe each preset falls back to when neither `--` args nor
/// `[probe].command` supply one. Node's depends on the lockfile at `path`.
fn default_probe(preset: Preset, path: &std::path::Path) -> Vec<String> {
    let cmd: &[&str] = match preset {
        Preset::Rust => &["cargo", "test"],
        Preset::Go => &["go", "test", "./..."],
        Preset::Node => PackageManager::detect(path).test_command(),
    };
    cmd.iter().map(ToString::to_string).collect()
}

fn probe_env_has_key(probe_env: &[runner::ProbeEnvTemplate], key: &str) -> bool {
    probe_env.iter().any(|t| t.key() == key)
}

/// Append one probe-env default when the user hasn't set `key` themselves;
/// returns the fill description for the preset debug line.
fn fill_probe_env(
    probe_env: &mut Vec<runner::ProbeEnvTemplate>,
    key: &str,
    value: &str,
) -> Option<String> {
    if probe_env_has_key(probe_env, key) {
        return None;
    }
    probe_env.push(runner::ProbeEnvTemplate::parse(key.to_string(), value));
    Some(format!("probe_env += {key}={value}"))
}

/// Append the rust preset's probe-env defaults to `probe_env` when the user
/// hasn't set the same key themselves. Returns a description of each fill for
/// the preset debug line.
///
/// Deliberately does NOT inject `RUSTC_WRAPPER=sccache` when sccache happens
/// to be on PATH: the preset must expand to the same run on every machine.
/// sccache stays opt-in via `--probe-env RUSTC_WRAPPER=sccache` (doctor
/// suggests this when it detects sccache).
fn rust_preset_probe_env_fills(probe_env: &mut Vec<runner::ProbeEnvTemplate>) -> Vec<String> {
    fill_probe_env(probe_env, "CARGO_TARGET_DIR", "{build_cache}")
        .into_iter()
        .collect()
}

/// Append the go preset's probe-env defaults to `probe_env` when the user
/// hasn't set the same keys themselves. Returns a description of each fill
/// for the preset debug line.
///
/// Both point into the shared build-cache dir (the runner creates it): the Go
/// build cache is concurrency-safe so workers share GOCACHE, and GOTMPDIR only
/// hosts per-invocation work dirs the `go` command creates itself — this keeps
/// probe temp writes out of the system /tmp without per-worker dirs.
fn go_preset_probe_env_fills(probe_env: &mut Vec<runner::ProbeEnvTemplate>) -> Vec<String> {
    let mut fills = Vec::new();
    fills.extend(fill_probe_env(probe_env, "GOCACHE", "{build_cache}/go-build"));
    fills.extend(fill_probe_env(probe_env, "GOTMPDIR", "{build_cache}"));
    fills
}

/// Append the node preset's probe-env defaults — the detected package
/// manager's cache dirs pointed into the shared build-cache dir — when the
/// user hasn't set the same keys themselves. Package-manager caches are safe
/// to share across workers (the workspace itself is isolated by the backend),
/// so no per-worker split.
fn node_preset_probe_env_fills(
    probe_env: &mut Vec<runner::ProbeEnvTemplate>,
    pm: PackageManager,
) -> Vec<String> {
    pm.cache_env_fills()
        .iter()
        .filter_map(|(key, value)| fill_probe_env(probe_env, key, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Commands};
    use clap::Parser as _;
    use std::path::Path;

    fn parse_args(extra: &[&str]) -> TestMutantsArgs {
        let mut argv = vec!["ooze", "test-mutants"];
        argv.extend_from_slice(extra);
        let cli = Cli::try_parse_from(argv).expect("args should parse");
        match cli.command {
            Commands::TestMutants(a) => *a,
            _ => unreachable!("parsed a test-mutants invocation"),
        }
    }

    /// Temp dir with a Cargo.toml and an empty ooze.toml, so resolution is
    /// hermetic (independent of the real repo's config and cwd).
    fn cargo_project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("ooze.toml"), "").unwrap();
        tmp
    }

    fn resolve_in(dir: &Path, extra: &[&str]) -> anyhow::Result<ResolvedTestMutants> {
        let path = dir.display().to_string();
        let config = dir.join("ooze.toml").display().to_string();
        let mut argv: Vec<&str> = vec!["--path", &path, "--config", &config];
        argv.extend_from_slice(extra);
        test_mutants(parse_args(&argv))
    }

    fn env_values(probe_env: &[runner::ProbeEnvTemplate], key: &str) -> Vec<String> {
        let ctx = runner::ProbeEnvCtx {
            worker: 0,
            build_cache: Some(Path::new("/bc")),
        };
        probe_env
            .iter()
            .filter(|t| t.key() == key)
            .map(|t| t.eval(ctx))
            .collect()
    }

    #[test]
    fn preset_rust_parses() {
        let args = parse_args(&["--preset", "rust"]);
        assert_eq!(args.preset, Some(Preset::Rust));
    }

    #[test]
    fn preset_unknown_fails_to_parse() {
        let Err(err) = Cli::try_parse_from(["ooze", "test-mutants", "--preset", "gopher"]) else {
            panic!("unknown preset must be rejected");
        };
        assert!(
            err.to_string().contains("gopher"),
            "error should name the bad value: {err}"
        );
    }

    #[test]
    fn rust_preset_defaults_to_worktree_backend() {
        let dir = cargo_project();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert_eq!(r.workspace_backend, WorkspaceBackendArg::Worktree);
    }

    #[test]
    fn explicit_backend_overrides_preset_default() {
        let dir = cargo_project();
        let r = resolve_in(
            dir.path(),
            &["--preset", "rust", "--workspace-backend", "overlay"],
        )
        .unwrap();
        assert_eq!(r.workspace_backend, WorkspaceBackendArg::Overlay);
    }

    #[test]
    fn config_backend_overrides_preset_default() {
        let dir = cargo_project();
        std::fs::write(
            dir.path().join("ooze.toml"),
            "[runner]\nworkspace_backend = \"copy\"\n",
        )
        .unwrap();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert_eq!(r.workspace_backend, WorkspaceBackendArg::Copy);
    }

    #[test]
    fn rust_preset_enables_per_worker_cache_and_warmup() {
        let dir = cargo_project();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert!(r.per_worker_cache);
        assert!(r.warmup);
    }

    #[test]
    fn config_can_disable_warmup_despite_preset() {
        let dir = cargo_project();
        std::fs::write(dir.path().join("ooze.toml"), "[runner]\nwarmup = false\n").unwrap();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert!(!r.warmup, "explicit config choice must win over preset");
        assert!(r.per_worker_cache, "unset option still gets preset default");
    }

    #[test]
    fn rust_preset_injects_cargo_target_dir() {
        let dir = cargo_project();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert_eq!(env_values(&r.probe_env, "CARGO_TARGET_DIR"), ["/bc"]);
    }

    #[test]
    fn rust_preset_keeps_existing_cargo_target_dir() {
        let dir = cargo_project();
        let r = resolve_in(
            dir.path(),
            &["--preset", "rust", "--probe-env", "CARGO_TARGET_DIR=custom"],
        )
        .unwrap();
        assert_eq!(
            env_values(&r.probe_env, "CARGO_TARGET_DIR"),
            ["custom"],
            "user value kept, no duplicate injected"
        );
    }

    #[test]
    fn rust_preset_defaults_probe_to_cargo_test() {
        let dir = cargo_project();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        assert_eq!(r.probe, ["cargo", "test"]);
    }

    #[test]
    fn explicit_probe_overrides_preset_default() {
        let dir = cargo_project();
        let r = resolve_in(
            dir.path(),
            &["--preset", "rust", "--", "cargo", "test", "--lib"],
        )
        .unwrap();
        assert_eq!(r.probe, ["cargo", "test", "--lib"]);
    }

    #[test]
    fn missing_probe_without_preset_still_errors() {
        let dir = cargo_project();
        let Err(err) = resolve_in(dir.path(), &[]) else {
            panic!("resolution without a probe must fail");
        };
        assert!(err.to_string().contains("missing probe command"));
    }

    #[test]
    fn rust_preset_errors_without_cargo_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("ooze.toml"), "").unwrap();
        let Err(err) = resolve_in(tmp.path(), &["--preset", "rust"]) else {
            panic!("rust preset outside a cargo project must fail");
        };
        assert!(
            err.to_string().contains("Cargo.toml"),
            "error should mention Cargo.toml: {err}"
        );
    }

    #[test]
    fn preset_fills_list_matches_resolution() {
        // `Preset::fills()` is what doctor advertises the preset will do;
        // verify every advertised fill against actual resolution in a bare
        // cargo project so the list can't drift from this module.
        let dir = cargo_project();
        let r = resolve_in(dir.path(), &["--preset", "rust"]).unwrap();
        for fill in Preset::Rust.fills(dir.path()) {
            match *fill {
                "probe=`cargo test`" => assert_eq!(r.probe, ["cargo", "test"]),
                "workspace_backend=worktree" => {
                    assert_eq!(r.workspace_backend, WorkspaceBackendArg::Worktree)
                }
                "per_worker_cache=true" => assert!(r.per_worker_cache),
                "warmup=true" => assert!(r.warmup),
                "probe_env += CARGO_TARGET_DIR={build_cache}" => {
                    assert_eq!(env_values(&r.probe_env, "CARGO_TARGET_DIR"), ["/bc"])
                }
                other => panic!(
                    "Preset::Rust.fills() advertises {other:?} but this test knows no \
                     matching fill in resolve; update fills() or extend this test"
                ),
            }
        }
    }

    #[test]
    fn probe_env_fills_only_cargo_target_dir() {
        let mut env = Vec::new();
        let fills = rust_preset_probe_env_fills(&mut env);
        assert_eq!(env_values(&env, "CARGO_TARGET_DIR"), ["/bc"]);
        assert_eq!(fills.len(), 1);
    }

    #[test]
    fn probe_env_fills_never_inject_rustc_wrapper() {
        // sccache is opt-in: the preset must expand identically whether or
        // not sccache is installed, so RUSTC_WRAPPER is never injected.
        let mut env = Vec::new();
        rust_preset_probe_env_fills(&mut env);
        assert!(env_values(&env, "RUSTC_WRAPPER").is_empty());
    }

    #[test]
    fn probe_env_fills_keep_existing_entries() {
        let mut env = vec![runner::ProbeEnvTemplate::parse(
            "RUSTC_WRAPPER".to_string(),
            "mine",
        )];
        rust_preset_probe_env_fills(&mut env);
        assert_eq!(env_values(&env, "RUSTC_WRAPPER"), ["mine"]);
    }
}
