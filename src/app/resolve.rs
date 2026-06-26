//! Resolution of `test-mutants` arguments into a fully-typed, runnable settings
//! struct. This is where CLI flags, the loaded `ooze.toml`, and built-in defaults
//! are merged exactly once, before any domain logic runs. The command body in
//! `super::run` then consumes [`ResolvedTestMutants`] without re-deriving anything.

use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{TestMutantsArgs, WorkspaceBackendArg};
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

    let (cfg, cfg_loaded_from) = config::load_config(config_path.as_deref())?;
    if let Some(p) = &cfg_loaded_from {
        eprintln!("ooze: loaded config from {}", p.display());
    }

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
    let per_worker_cache = resolve_bool_flag(per_worker_cache, cfg.runner.per_worker_cache);
    let warmup = resolve_bool_flag(warmup, cfg.runner.warmup);
    let workspace_backend = match workspace_backend {
        Some(w) => w,
        None => match cfg.runner.workspace_backend.as_deref() {
            Some(s) => parse_workspace_backend_str(s)?,
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

    let probe_env: Vec<runner::ProbeEnvTemplate> = resolve_probe_env(probe_env, &cfg.probe.env)?
        .into_iter()
        .map(|(k, v)| runner::ProbeEnvTemplate::parse(k, &v))
        .collect();

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
        } else {
            anyhow::bail!(
                "missing probe command; pass one after `--` or set [probe].command in ooze.toml"
            );
        }
    }

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
    })
}
