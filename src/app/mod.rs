//! Command orchestration. `main.rs` only parses args and calls [`run`]; the CLI
//! surface lives in `crate::cli` and everything else lives here.

mod resolve;

use crate::cli::{Cli, Commands, WorkspaceBackendArg, parse_key_val};
use crate::{
    config, core, doctor, execution, lang, ledger, mutate, planning, probe, report, scheduler,
    skip, workspace,
};

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, ValueEnum};

/// Print how well the coverage reports line up with the scanned source tree.
fn print_coverage_diagnostics(diagnostics: &planning::CoverageDiagnostics) {
    let m = &diagnostics.matches;
    eprintln!("ooze: coverage reports parsed: {}", diagnostics.reports);
    eprintln!("ooze: coverage source files:  {}", m.coverage_source_files);
    eprintln!("ooze: matched source files:   {}", m.matched_source_files);
    eprintln!(
        "ooze: unmatched coverage files: {}",
        m.unmatched_coverage_files
    );
    eprintln!(
        "ooze: unmatched scanned files:  {}",
        m.unmatched_source_files
    );
    if m.coverage_source_files > 0 && m.matched_source_files == 0 {
        eprintln!(
            "ooze: warning: no scanned files matched any coverage entry — check path roots (Docker/CI/monorepo prefixes)"
        );
    }
}

fn prompt_language() -> anyhow::Result<String> {
    use std::io::{BufRead, Write};
    eprintln!("Select a language preset:");
    for (i, (key, label)) in config::LANGUAGES.iter().enumerate() {
        eprintln!("  [{}] {} ({})", i + 1, label, key);
    }
    eprint!("Choice [1-{}]: ", config::LANGUAGES.len());
    std::io::stderr().flush()?;
    let mut input = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut input)
        .context("reading language choice from stdin")?;
    let trimmed = input.trim();
    if let Ok(n) = trimmed.parse::<usize>() {
        return config::LANGUAGES
            .get(n.wrapping_sub(1))
            .map(|(k, _)| k.to_string())
            .ok_or_else(|| anyhow::anyhow!("choice must be 1-{}", config::LANGUAGES.len()));
    }
    if config::LANGUAGES.iter().any(|(k, _)| *k == trimmed) {
        return Ok(trimmed.to_string());
    }
    let known: Vec<&str> = config::LANGUAGES.iter().map(|(k, _)| *k).collect();
    anyhow::bail!("unknown language {trimmed:?}; known: {}", known.join(", "))
}

fn resolve_bool_flag(cli_flag: bool, config_value: Option<bool>) -> bool {
    cli_flag || config_value == Some(true)
}

// Used when the config key is a positive "enabled" flag but the CLI exposes the negative
// ("no_static_skips"): the flag is on when the CLI says so OR the config says disabled.
fn resolve_disabled_flag(cli_flag: bool, config_enabled: Option<bool>) -> bool {
    cli_flag || config_enabled == Some(false)
}

// Returns per-worker build-cache paths when jobs > 1; empty otherwise.
fn per_worker_cache_dirs(jobs: usize, cache_dir: &std::path::Path) -> Vec<PathBuf> {
    if jobs > 1 {
        (0..jobs)
            .map(|i| cache_dir.join(format!("build-cache-job-{i}")))
            .collect()
    } else {
        Vec::new()
    }
}

fn looks_like_path(s: &str) -> bool {
    s.contains('/') || s.starts_with('.') || std::path::Path::new(s).is_absolute()
}

// Builds the resolved report-size options from the detail baseline, then applies
// the individual --no-* / --only-survivors overrides (CLI flag OR config value).
// The flag pairs mirror the CLI surface, so several bools are expected here.
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn build_report_options(
    detail: report::ReportDetail,
    no_diff: bool,
    cfg_diff: Option<bool>,
    no_stdout: bool,
    cfg_stdout: Option<bool>,
    no_stderr: bool,
    cfg_stderr: Option<bool>,
    only_survivors: bool,
    cfg_only_survivors: Option<bool>,
) -> report::ReportOptions {
    let mut report_opts = report::ReportOptions::from_detail(detail);
    if resolve_disabled_flag(no_diff, cfg_diff) {
        report_opts.include_diff = false;
    }
    if resolve_disabled_flag(no_stdout, cfg_stdout) {
        report_opts.include_stdout = false;
    }
    if resolve_disabled_flag(no_stderr, cfg_stderr) {
        report_opts.include_stderr = false;
    }
    if resolve_bool_flag(only_survivors, cfg_only_survivors) {
        report_opts.only_survivors = true;
    }
    report_opts
}

// CLI excludes take precedence; fall back to config scope excludes only when
// none were passed on the command line.
fn resolve_exclude_list(cli: Vec<String>, cfg: &[String]) -> Vec<String> {
    let mut exclude = cli;
    if exclude.is_empty() {
        exclude.extend(cfg.iter().cloned());
    }
    exclude
}

// CLI probe-env pairs take precedence; otherwise parse the config entries.
fn resolve_probe_env(
    cli: Vec<(String, String)>,
    cfg: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let mut probe_env = cli;
    if probe_env.is_empty() {
        for entry in cfg {
            probe_env.push(parse_key_val(entry).map_err(|e| anyhow::anyhow!(e))?);
        }
    }
    Ok(probe_env)
}

// CLI operators take precedence; otherwise expand config operators and
// categories, de-duplicating operators contributed by multiple categories.
fn resolve_operators(
    cli: Vec<core::OperatorName>,
    cfg_operators: Option<&Vec<String>>,
    cfg_categories: Option<&Vec<String>>,
) -> anyhow::Result<Vec<core::OperatorName>> {
    let mut operators = cli;
    if operators.is_empty() {
        if let Some(ops) = cfg_operators {
            for s in ops {
                operators.push(core::OperatorName::parse(s).ok_or_else(|| {
                    anyhow::anyhow!("unknown operator {s:?} in [mutation].operators")
                })?);
            }
        }
        if let Some(cats) = cfg_categories {
            for s in cats {
                let cat = core::OperatorCategory::parse(s).ok_or_else(|| {
                    anyhow::anyhow!("unknown category {s:?} in [mutation].categories")
                })?;
                for op in cat.operators() {
                    if !operators.contains(&op) {
                        operators.push(op);
                    }
                }
            }
        }
    }
    Ok(operators)
}

// CLI exclude-operators take precedence; otherwise expand config
// exclude_operators and exclude_categories, de-duplicating.
fn resolve_exclude_operators(
    cli: Vec<core::OperatorName>,
    cfg_exclude_operators: &[String],
    cfg_exclude_categories: &[String],
) -> anyhow::Result<Vec<core::OperatorName>> {
    let mut exclude_operators = cli;
    if exclude_operators.is_empty() {
        for s in cfg_exclude_operators {
            exclude_operators.push(core::OperatorName::parse(s).ok_or_else(|| {
                anyhow::anyhow!("unknown operator {s:?} in [mutation].exclude_operators")
            })?);
        }
        for s in cfg_exclude_categories {
            let cat = core::OperatorCategory::parse(s).ok_or_else(|| {
                anyhow::anyhow!("unknown category {s:?} in [mutation].exclude_categories")
            })?;
            for op in cat.operators() {
                if !exclude_operators.contains(&op) {
                    exclude_operators.push(op);
                }
            }
        }
    }
    Ok(exclude_operators)
}

// Worker count: one per worker build-cache dir when per-worker caching is on,
// otherwise the requested job count (at least one).
fn num_workers(jobs: usize, worker_build_cache_dirs: &[PathBuf]) -> usize {
    if worker_build_cache_dirs.is_empty() {
        jobs.max(1)
    } else {
        worker_build_cache_dirs.len()
    }
}

// Computes the {worker}-templated probe-env directories that should be created,
// one per worker index, skipping env values that are not `{worker}`-templated or
// not path-like.
fn worker_probe_env_dirs(
    probe_env: &[execution::ProbeEnvTemplate],
    num_workers: usize,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for t in probe_env {
        if !t.references_worker() {
            continue;
        }
        for i in 0..num_workers {
            let resolved = t.eval(execution::ProbeEnvCtx {
                worker: i,
                build_cache: None,
            });
            if looks_like_path(&resolved) {
                dirs.push(PathBuf::from(resolved));
            }
        }
    }
    dirs
}

// `Some` reference for `BatchConfig` when per-worker dirs exist, else `None`.
fn worker_build_cache_arg(dirs: &[PathBuf]) -> Option<&[PathBuf]> {
    if dirs.is_empty() { None } else { Some(dirs) }
}

// Warmup dispatch decision: warm per-worker caches, a single shared cache dir,
// or nothing to do.
#[derive(Debug, PartialEq, Eq)]
enum WarmupTarget<'a> {
    Workers,
    Shared(&'a std::path::Path),
    Nothing,
}

fn warmup_target<'a>(
    worker_build_cache_dirs: &[PathBuf],
    target_dir: Option<&'a std::path::Path>,
) -> WarmupTarget<'a> {
    if !worker_build_cache_dirs.is_empty() {
        WarmupTarget::Workers
    } else if let Some(dir) = target_dir {
        WarmupTarget::Shared(dir)
    } else {
        WarmupTarget::Nothing
    }
}

// Maps a warmup probe exit status into a result, failing on non-success.
fn warmup_status_to_result(status: std::process::ExitStatus) -> anyhow::Result<()> {
    if !status.success() {
        anyhow::bail!("warmup command failed with status {status}");
    }
    Ok(())
}

// Per-mutant progress is shown unless quieted, and only when the resolved mode
// allows it.
fn progress_enabled(quiet: bool, progress_resolved: bool) -> bool {
    !quiet && progress_resolved
}

/// Run the `[runner].pre_run` command once from the project root, inheriting
/// stdio so the user sees its output live. Fails the run on a nonzero exit.
fn run_pre_run_command(cmd: &[String], path: &std::path::Path) -> anyhow::Result<()> {
    eprintln!("ooze: pre-run: {}", cmd.join(" "));
    let status = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(path)
        .status()
        .with_context(|| format!("spawning pre_run command {:?}", cmd[0]))?;
    if !status.success() {
        anyhow::bail!("pre_run command {:?} failed with {status}", cmd.join(" "));
    }
    Ok(())
}

fn parse_report_format_str(s: &str) -> anyhow::Result<report::ReportFormat> {
    <report::ReportFormat as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid report format {s:?}: {e}"))
}

fn parse_strategy_str(s: &str) -> anyhow::Result<scheduler::MutationStrategy> {
    <scheduler::MutationStrategy as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid strategy {s:?}: {e}"))
}

fn parse_workspace_backend_str(s: &str) -> anyhow::Result<WorkspaceBackendArg> {
    <WorkspaceBackendArg as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid workspace_backend {s:?}: {e}"))
}

fn parse_report_detail_str(s: &str) -> anyhow::Result<report::ReportDetail> {
    <report::ReportDetail as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid report detail {s:?}: {e}"))
}

#[derive(serde::Serialize)]
struct PlannedCandidate {
    /// Position in the deterministic selection order (0-based). This is the plan
    /// order, not the eventual completion order — parallel execution may finish
    /// mutants in any order.
    plan_index: usize,
    #[serde(flatten)]
    candidate: core::MutationCandidate,
    #[serde(flatten)]
    selection: scheduler::SelectionExplanation,
    /// Stable, discovery-independent identity used for seeded ranking. Present
    /// only for seeded runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    stable_id: Option<String>,
    /// Hex of the BLAKE3 ranking key, for debugging seeded order. Present only
    /// for seeded runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    ranking_key: Option<String>,
}

#[derive(serde::Serialize)]
struct Plan {
    total_candidates: usize,
    skipped: usize,
    selected: usize,
    /// Candidate universe the selection ranked over (before `--limit`).
    candidate_count: usize,
    /// Selected mutant count after `--limit`. Mirrors `selected`; named to match
    /// the documented selection-metadata contract.
    selected_count: usize,
    strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<String>,
    /// Selection algorithm name, present only when a seed was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    selection_algorithm: Option<&'static str>,
    excluded_patterns: Vec<String>,
    operator_filter: mutate::OperatorFilterReport,
    candidates: Vec<PlannedCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_candidates: Option<Vec<skip::SkippedCandidate>>,
}

/// Serializable projection of the selected plan, persisted to the run ledger
/// as `plan.json`. Mirrors the `plan-mutants` output shape minus per-candidate
/// selection explanations.
#[derive(serde::Serialize)]
struct RunPlanSnapshot<'a> {
    total_candidates: usize,
    skipped: usize,
    selected: usize,
    candidate_count: usize,
    selected_count: usize,
    strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selection_algorithm: Option<&'static str>,
    excluded_patterns: &'a [String],
    operator_filter: &'a mutate::OperatorFilterReport,
    candidates: &'a [core::MutationCandidate],
}

/// Parse the command line and execute the selected command.
pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { path, format } => {
            let spans = lang::scan_directory(std::path::Path::new(&path))?;
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&spans)?);
            }
        }
        Commands::Mutants {
            path,
            format,
            exclude,
        } => {
            let excludes = planning::resolve_excludes(&path, &exclude);
            let registry = lang::CompiledRegistry::compile(
                lang::supported_languages(),
                &mutate::OperatorFilter::allow_all(),
            )?;
            let functions = lang::scan_directory_with_registry(&registry, &path, &excludes)?;
            let candidates = mutate::discover_mutants(&functions, &registry)?;
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
            }
        }
        Commands::Operators { format } => {
            #[derive(serde::Serialize)]
            struct OperatorEntry {
                #[serde(flatten)]
                info: core::OperatorInfo,
                /// Languages with a registered implementation of this operator.
                languages: Vec<core::Language>,
            }
            let infos: Vec<OperatorEntry> = core::OperatorName::ALL
                .iter()
                .copied()
                .map(|op| {
                    let mut languages: Vec<core::Language> = mutate::registry::all()
                        .filter(|m| m.operator == op)
                        .map(|m| m.language)
                        .collect();
                    languages.sort();
                    languages.dedup();
                    OperatorEntry {
                        info: op.info(),
                        languages,
                    }
                })
                .collect();
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&infos)?);
            } else {
                for entry in &infos {
                    let info = &entry.info;
                    let langs = if entry.languages.is_empty() {
                        "(none)".to_string()
                    } else {
                        entry
                            .languages
                            .iter()
                            .map(|l| l.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    println!(
                        "{:<18} [{}] default_enabled={}\n  {}\n  langs: {}\n  hint: {}\n",
                        info.name,
                        info.category,
                        info.default_enabled,
                        info.description,
                        langs,
                        info.test_hint
                    );
                }
            }
        }
        Commands::Languages { format } => {
            #[derive(serde::Serialize)]
            struct LanguageInfo {
                language: core::Language,
                support: core::SupportLevel,
                mutates: bool,
                operators: usize,
                extensions: &'static [&'static str],
            }
            let infos: Vec<LanguageInfo> = lang::supported_languages()
                .iter()
                .map(|g| LanguageInfo {
                    language: g.id,
                    support: g.support,
                    mutates: g.support.mutates(),
                    operators: g.mutators.len(),
                    extensions: g.extensions,
                })
                .collect();
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&infos)?);
            } else {
                for info in &infos {
                    println!(
                        "{:<12} {:<20} {:>2} operators  [{}]",
                        info.language.as_str(),
                        info.support.as_str(),
                        info.operators,
                        info.extensions.join(", "),
                    );
                }
            }
        }
        Commands::PlanMutants {
            path,
            lcov,
            coverage,
            strategy,
            limit,
            seed,
            format,
            exclude,
            changed_only,
            operators,
            exclude_operators,
            no_static_skips,
            show_skipped,
        } => {
            let excludes = planning::resolve_excludes(&path, &exclude);
            let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);
            let built = planning::build_plan(planning::PlanOptions {
                path,
                excludes,
                filter,
                strategy,
                limit,
                seed,
                changed_only,
                no_static_skips,
                coverage,
                lcov,
            })?;
            if let Some(diagnostics) = &built.coverage_diagnostics {
                print_coverage_diagnostics(diagnostics);
            }

            let selection_ctx = built.selection.as_ref();
            let planned: Vec<PlannedCandidate> = built
                .candidates
                .into_iter()
                .enumerate()
                .map(|(plan_index, c)| {
                    let selection = scheduler::explain(built.strategy, &c, &built.crap_entries);
                    let (stable_id, ranking_key) = selection_ctx.map_or((None, None), |ctx| {
                        (
                            Some(crate::selection::stable_candidate_id(&c)),
                            Some(ctx.ranking_key_hex(&c)),
                        )
                    });
                    PlannedCandidate {
                        plan_index,
                        candidate: c,
                        selection,
                        stable_id,
                        ranking_key,
                    }
                })
                .collect();

            let plan = Plan {
                total_candidates: built.total_candidates_before_static_skips,
                skipped: built.skipped_candidates.len(),
                selected: planned.len(),
                candidate_count: built.candidate_count,
                selected_count: planned.len(),
                strategy: format!("{:?}", built.strategy).to_lowercase(),
                seed: built.seed,
                selection_algorithm: selection_ctx.map(|_| crate::selection::ALGORITHM_NAME),
                excluded_patterns: built.excludes,
                operator_filter: built.operator_filter,
                candidates: planned,
                skipped_candidates: if show_skipped {
                    Some(built.skipped_candidates)
                } else {
                    None
                },
            };

            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        Commands::ApplyMutant { path, id } => {
            let repo_root = path;

            let registry = lang::CompiledRegistry::compile(
                lang::supported_languages(),
                &mutate::OperatorFilter::allow_all(),
            )?;
            let functions = lang::scan_directory_with_registry(&registry, &repo_root, &[])?;
            let candidates = mutate::discover_mutants(&functions, &registry)?;

            let Some(candidate) = candidates.into_iter().find(|c| c.id == id) else {
                anyhow::bail!("no mutation candidate found with id {id:?}");
            };

            let workspace = workspace::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;

            println!("{}", applied.diff);
        }
        Commands::TestMutants(args) => {
            let resolve::ResolvedTestMutants {
                path,
                strategy,
                limit,
                seed,
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
            } = resolve::test_mutants(*args)?;

            if let Some(cmd) = &pre_run {
                run_pre_run_command(cmd, &path)?;
            }

            let built = planning::build_plan(planning::PlanOptions {
                path: path.clone(),
                excludes,
                filter,
                strategy,
                limit,
                seed,
                changed_only,
                no_static_skips,
                coverage,
                lcov,
            })?;
            if let Some(stats) = &built.changed_only {
                eprintln!(
                    "ooze: --changed-only {}: {} of {} candidates in changed files",
                    stats.base, stats.kept, stats.before
                );
            }
            if let Some(diagnostics) = &built.coverage_diagnostics {
                print_coverage_diagnostics(diagnostics);
            }
            let crap_entries = built.crap_entries;
            let candidates = built.candidates;

            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;

            let cache_dir = if cache_dir.is_absolute() {
                cache_dir
            } else {
                repo_root.join(&cache_dir)
            };
            let runs_dir = if runs_dir.is_absolute() {
                runs_dir
            } else {
                repo_root.join(&runs_dir)
            };
            std::fs::create_dir_all(&cache_dir)
                .with_context(|| format!("creating cache dir {}", cache_dir.display()))?;
            std::fs::create_dir_all(&runs_dir)
                .with_context(|| format!("creating runs dir {}", runs_dir.display()))?;

            let (target_dir, worker_build_cache_dirs): (Option<PathBuf>, Vec<PathBuf>) =
                if per_worker_cache {
                    let dirs = per_worker_cache_dirs(jobs, &cache_dir);
                    for d in &dirs {
                        std::fs::create_dir_all(d).with_context(|| {
                            format!("creating worker build cache dir {}", d.display())
                        })?;
                    }
                    (None, dirs)
                } else {
                    let dir = build_cache_dir
                        .unwrap_or_else(|| workspace::default_build_cache_dir(&cache_dir));
                    std::fs::create_dir_all(&dir)
                        .with_context(|| format!("creating build cache dir {}", dir.display()))?;
                    (Some(dir), Vec::new())
                };

            let num_workers = num_workers(jobs, &worker_build_cache_dirs);
            for p in worker_probe_env_dirs(&probe_env, num_workers) {
                std::fs::create_dir_all(&p)
                    .with_context(|| format!("creating probe-env directory {}", p.display()))?;
            }

            let backend = workspace_backend.resolve(&repo_root);

            // Persistent run ledger, written for every report format so the
            // run stays inspectable after exit. A ledger failure fails the
            // run: agents relying on it are worse off with a silent skip.
            let run_ledger = ledger::RunLedger::create(
                &runs_dir,
                ledger::RunMetadata {
                    run_id: ledger::new_run_id(),
                    started_at: ledger::utc_timestamp(),
                    repo_root: repo_root.clone(),
                    jobs,
                    format: format
                        .to_possible_value()
                        .expect("report format has a CLI name")
                        .get_name()
                        .to_string(),
                    probe: probe.as_vec(),
                    strategy: format!("{:?}", built.strategy).to_lowercase(),
                    limit,
                    seed: built.seed.clone(),
                    selection_algorithm: built
                        .selection
                        .as_ref()
                        .map(|_| crate::selection::ALGORITHM_NAME.to_string()),
                    workspace_backend: format!("{backend:?}").to_lowercase(),
                },
            )?;
            run_ledger.write_plan(&RunPlanSnapshot {
                total_candidates: built.total_candidates_before_static_skips,
                skipped: built.skipped_candidates.len(),
                selected: candidates.len(),
                candidate_count: built.candidate_count,
                selected_count: candidates.len(),
                strategy: format!("{:?}", built.strategy).to_lowercase(),
                seed: built.seed.as_deref(),
                selection_algorithm: built
                    .selection
                    .as_ref()
                    .map(|_| crate::selection::ALGORITHM_NAME),
                excluded_patterns: &built.excludes,
                operator_filter: &built.operator_filter,
                candidates: &candidates,
            })?;
            if matches!(format, report::ReportFormat::Human) {
                eprintln!("ooze: run ledger: {}", run_ledger.dir().display());
            }

            // One worktree per worker, created up front and reused across
            // mutants (cleaned up explicitly below: process::exit skips Drop).
            // Created before preflight/warmup so those run inside the same
            // workspaces the probes will use — running them in the repo root
            // could reuse cache entries left by a previous run's last mutant.
            let mut worktree_pool = if backend == workspace::WorkspaceBackend::Worktree {
                let workers = jobs.max(1);
                eprintln!(
                    "creating {workers} git worktree(s) under {}",
                    runs_dir.join("worktrees").display()
                );
                Some(workspace::worktree::WorktreePool::create(
                    &repo_root, &runs_dir, workers,
                )?)
            } else {
                None
            };

            let baseline_root = worktree_pool
                .as_ref()
                .map_or_else(|| repo_root.clone(), |p| p.path_for(0));

            if preflight {
                let preflight_build_cache = target_dir.as_deref().or_else(|| {
                    worker_build_cache_dirs
                        .first()
                        .map(std::path::PathBuf::as_path)
                });
                let preflight_envs = execution::template::eval_all(
                    &probe_env,
                    execution::ProbeEnvCtx {
                        worker: 0,
                        build_cache: preflight_build_cache,
                    },
                );
                let outcome =
                    execution::preflight(&baseline_root, &probe, timeout, &preflight_envs)?;
                if !outcome.success {
                    #[derive(serde::Serialize)]
                    struct PreflightFailure {
                        error: &'static str,
                        message: &'static str,
                        exit_code: Option<i32>,
                        duration_ms: u128,
                        stdout: String,
                        stderr: String,
                    }
                    let (err, msg) = if outcome.timed_out {
                        (
                            "preflight_timeout",
                            "Probe timed out on unmodified code; mutation results would be invalid.",
                        )
                    } else {
                        (
                            "preflight_failed",
                            "Probe failed on unmodified code; mutation results would be invalid.",
                        )
                    };
                    let payload = PreflightFailure {
                        error: err,
                        message: msg,
                        exit_code: outcome.exit_code,
                        duration_ms: outcome.duration_ms,
                        stdout: outcome.stdout,
                        stderr: outcome.stderr,
                    };
                    if matches!(format, report::ReportFormat::Human) {
                        eprintln!("Preflight failed.\n");
                        eprintln!("{msg}\n");
                        eprintln!("Command: {}", probe.display());
                        if let Some(code) = payload.exit_code {
                            eprintln!("Exit code: {code}");
                        }
                    } else if matches!(format, report::ReportFormat::Jsonl) {
                        // Keep jsonl stdout newline-delimited: one compact line.
                        println!("{}", serde_json::to_string(&payload)?);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    }
                    // process::exit skips Drop, so release the worktrees here.
                    if let Some(pool) = worktree_pool.as_mut()
                        && let Err(e) = pool.cleanup()
                    {
                        eprintln!("warning: failed to clean up git worktrees: {e:#}");
                    }
                    std::process::exit(report::OozeExitCode::PreflightFailed.code());
                }
            }

            if warmup {
                match warmup_target(&worker_build_cache_dirs, target_dir.as_deref()) {
                    WarmupTarget::Workers => {
                        eprintln!(
                            "warming up {} worker build cache dirs...",
                            worker_build_cache_dirs.len()
                        );
                        let warmup_workspaces: Vec<PathBuf> = (0..worker_build_cache_dirs.len())
                            .map(|i| {
                                worktree_pool
                                    .as_ref()
                                    .map_or_else(|| repo_root.clone(), |p| p.path_for(i))
                            })
                            .collect();
                        execution::warmup_workers(
                            &warmup_workspaces,
                            &probe,
                            &worker_build_cache_dirs,
                            jobs,
                            &probe_env,
                        )?;
                    }
                    WarmupTarget::Shared(dir) => {
                        eprintln!("warming up shared build cache dir...");
                        let extra = execution::template::eval_all(
                            &probe_env,
                            execution::ProbeEnvCtx {
                                worker: 0,
                                build_cache: Some(dir),
                            },
                        );
                        let status = execution::warmup(&baseline_root, &probe, Some(dir), &extra)?;
                        warmup_status_to_result(status)?;
                    }
                    WarmupTarget::Nothing => {}
                }
            }

            let progress_cb: Option<fn(execution::ProgressEvent<'_>)> = if progress_enabled {
                Some(|ev: execution::ProgressEvent<'_>| {
                    let status = match ev.outcome.status {
                        core::MutantStatus::Killed => "killed",
                        core::MutantStatus::Survived => "SURVIVED",
                        core::MutantStatus::Timeout => "timeout",
                        core::MutantStatus::Error => "ERROR",
                    };
                    eprintln!(
                        "[{}/{}] {} {}",
                        ev.completed, ev.total, status, ev.outcome.candidate.id
                    );
                })
            } else {
                None
            };

            // Event sink: every event is appended to the ledger; for jsonl it
            // is also streamed to stdout, one JSON object per line. Parallel
            // workers emit concurrently, so writes go through mutexes. The
            // sink is a plain `Fn`, so ledger write errors are parked in a
            // slot and checked after execution.
            let jsonl_stdout = matches!(format, report::ReportFormat::Jsonl);
            let jsonl_out = std::sync::Mutex::new(std::io::stdout());
            let event_error: std::sync::Mutex<Option<anyhow::Error>> = std::sync::Mutex::new(None);
            let event_sink = |event: execution::ExecutionEvent| {
                if jsonl_stdout {
                    use std::io::Write;
                    let line = serde_json::to_string(&event).expect("serialize execution event");
                    let mut out = jsonl_out.lock().expect("lock stdout for jsonl event");
                    writeln!(out, "{line}").expect("write jsonl event");
                }
                if let Err(err) = run_ledger.append_event(&event) {
                    let mut slot = event_error
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if slot.is_none() {
                        *slot = Some(err);
                    }
                }
            };
            let events: Option<execution::EventSink<'_>> = Some(&event_sink);

            let cfg = execution::BatchConfig {
                backend,
                timeout,
                build_cache_dir: target_dir.as_deref(),
                worker_build_cache_dirs: worker_build_cache_arg(&worker_build_cache_dirs),
                probe_env_templates: &probe_env,
                runs_dir: &runs_dir,
                progress: progress_cb,
                events,
                worktree_pool: worktree_pool.as_ref(),
            };

            let raw_report =
                execution::run_mutants_parallel(&repo_root, candidates, &probe, jobs, &cfg)?;

            if let Some(pool) = worktree_pool.as_mut()
                && let Err(e) = pool.cleanup()
            {
                eprintln!("warning: failed to clean up git worktrees: {e:#}");
            }

            if let Some(err) = event_error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                return Err(err.context("writing run ledger events"));
            }

            let mut enriched = report::enrich(raw_report, &crap_entries, &repo_root, context_lines);
            // The ledger keeps the full diagnostic report; user report
            // options (compact/only-survivors/…) only shape stdout output.
            run_ledger.write_report(&enriched)?;
            report::apply_options(&mut enriched, report_opts);

            match output.as_deref() {
                Some(path) => {
                    let text = format.render(&enriched)?;
                    std::fs::write(path, &text)
                        .with_context(|| format!("writing report to {}", path.display()))?;
                }
                // For jsonl the event stream already on stdout *is* the
                // output; the summary was emitted as `run_finished`.
                None if format == report::ReportFormat::Jsonl => {}
                None => print!("{}", format.render(&enriched)?),
            }

            let exit =
                report::exit_code_for_report(&enriched, no_fail_on_survivors, allow_incomplete);
            std::process::exit(exit.code());
        }
        Commands::Warmup {
            path,
            cache_dir,
            probe,
        } => {
            let probe = probe::ProbeCommand::new(probe)?;
            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;
            let cache_dir = if cache_dir.is_absolute() {
                cache_dir
            } else {
                repo_root.join(&cache_dir)
            };
            let target_dir = workspace::default_build_cache_dir(&cache_dir);
            let status = execution::warmup(&repo_root, &probe, Some(&target_dir), &[])?;
            warmup_status_to_result(status)?;
        }
        Commands::TestMutant { path, id, probe } => {
            let probe = probe::ProbeCommand::new(probe)?;
            let registry = lang::CompiledRegistry::compile(
                lang::supported_languages(),
                &mutate::OperatorFilter::allow_all(),
            )?;
            let functions = lang::scan_directory_with_registry(&registry, &path, &[])?;
            let candidates = mutate::discover_mutants(&functions, &registry)?;

            let Some(candidate) = candidates.into_iter().find(|c| c.id == id) else {
                anyhow::bail!("no mutation candidate found with id {id:?}");
            };

            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;

            let workspace = workspace::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;
            let outcome = execution::run_probe(workspace.path(), applied, &probe, None, &[])?;

            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Commands::Doctor {
            path,
            format,
            operators,
        } => {
            let report = doctor::run(&path, operators);
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::print_human(&report);
            }
            if report.has_failures() {
                std::process::exit(1);
            }
        }
        Commands::InitConfig {
            path,
            force,
            language,
        } => {
            if path.exists() && !force {
                anyhow::bail!(
                    "{} already exists; pass --force to overwrite",
                    path.display()
                );
            }
            let lang = match language {
                Some(l) => l,
                None => prompt_language()?,
            };
            let template = config::template_for_language(&lang).ok_or_else(|| {
                let known: Vec<&str> = config::LANGUAGES.iter().map(|(k, _)| *k).collect();
                anyhow::anyhow!("unknown language {lang:?}; known: {}", known.join(", "))
            })?;
            std::fs::write(&path, template)
                .with_context(|| format!("writing {}", path.display()))?;
            eprintln!("wrote {} ({})", path.display(), lang);
        }
        Commands::Crap {
            path,
            lcov,
            coverage,
            format,
        } => {
            let functions = lang::scan_directory(std::path::Path::new(&path))?;
            let coverage = planning::resolve_coverage(&coverage, lcov.as_deref())?;
            let (entries, diagnostics) =
                planning::score_with_optional_coverage(functions, coverage);
            if let Some(diagnostics) = &diagnostics {
                print_coverage_diagnostics(diagnostics);
            }
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_bool_flag_true_when_cli_set() {
        assert!(resolve_bool_flag(true, None));
        assert!(resolve_bool_flag(true, Some(false)));
        assert!(resolve_bool_flag(true, Some(true)));
    }

    #[test]
    fn resolve_bool_flag_true_when_config_enabled() {
        assert!(resolve_bool_flag(false, Some(true)));
    }

    #[test]
    fn resolve_bool_flag_false_when_neither() {
        assert!(!resolve_bool_flag(false, None));
        assert!(!resolve_bool_flag(false, Some(false)));
    }

    #[test]
    fn resolve_disabled_flag_true_when_cli_set() {
        assert!(resolve_disabled_flag(true, None));
        assert!(resolve_disabled_flag(true, Some(true)));
    }

    #[test]
    fn resolve_disabled_flag_true_when_config_disables() {
        assert!(resolve_disabled_flag(false, Some(false)));
    }

    #[test]
    fn resolve_disabled_flag_false_when_neither() {
        assert!(!resolve_disabled_flag(false, None));
        assert!(!resolve_disabled_flag(false, Some(true)));
    }

    #[test]
    fn per_worker_cache_dirs_multiple_jobs_returns_numbered_dirs() {
        let base = std::path::Path::new("/cache");
        let dirs = per_worker_cache_dirs(2, base);
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0], base.join("build-cache-job-0"));
        assert_eq!(dirs[1], base.join("build-cache-job-1"));
    }

    #[test]
    fn per_worker_cache_dirs_single_job_returns_empty() {
        let dirs = per_worker_cache_dirs(1, std::path::Path::new("/cache"));
        assert!(dirs.is_empty());
    }

    #[test]
    fn looks_like_path_slash_is_path() {
        assert!(looks_like_path("dir/file"));
    }

    #[test]
    fn looks_like_path_dot_prefix_is_path() {
        assert!(looks_like_path(".hidden"));
    }

    #[test]
    fn looks_like_path_absolute_is_path() {
        assert!(looks_like_path("/absolute/path"));
    }

    #[test]
    fn looks_like_path_plain_name_is_not_path() {
        assert!(!looks_like_path("plain"));
    }

    // --- build_report_options ---------------------------------------------

    #[allow(clippy::fn_params_excessive_bools)]
    fn report_opts(
        no_diff: bool,
        no_stdout: bool,
        no_stderr: bool,
        only_survivors: bool,
    ) -> report::ReportOptions {
        build_report_options(
            report::ReportDetail::Full,
            no_diff,
            None,
            no_stdout,
            None,
            no_stderr,
            None,
            only_survivors,
            None,
        )
    }

    #[test]
    fn build_report_options_disables_diff_when_requested() {
        // Full baseline includes diffs; the --no-diff flag must turn it off.
        assert!(!report_opts(true, false, false, false).include_diff);
        // ...and leave it on otherwise (guards the `= false` assignment).
        assert!(report_opts(false, false, false, false).include_diff);
    }

    #[test]
    fn build_report_options_disables_stdout_when_requested() {
        assert!(!report_opts(false, true, false, false).include_stdout);
        assert!(report_opts(false, false, false, false).include_stdout);
    }

    #[test]
    fn build_report_options_disables_stderr_when_requested() {
        assert!(!report_opts(false, false, true, false).include_stderr);
        assert!(report_opts(false, false, false, false).include_stderr);
    }

    #[test]
    fn build_report_options_enables_only_survivors_when_requested() {
        // Full baseline keeps all outcomes; --only-survivors flips it on.
        assert!(report_opts(false, false, false, true).only_survivors);
        assert!(!report_opts(false, false, false, false).only_survivors);
    }

    #[test]
    fn build_report_options_honors_config_values() {
        let opts = build_report_options(
            report::ReportDetail::Full,
            false,
            Some(false), // [report].diff = false
            false,
            Some(false),
            false,
            Some(false),
            false,
            Some(true), // [report].only_survivors = true
        );
        assert!(!opts.include_diff);
        assert!(!opts.include_stdout);
        assert!(!opts.include_stderr);
        assert!(opts.only_survivors);
    }

    // --- resolve_exclude_list ---------------------------------------------

    #[test]
    fn resolve_exclude_list_uses_config_when_cli_empty() {
        let out = resolve_exclude_list(vec![], &["cfg/**".to_string()]);
        assert_eq!(out, vec!["cfg/**".to_string()]);
    }

    #[test]
    fn resolve_exclude_list_keeps_cli_and_ignores_config() {
        let out = resolve_exclude_list(vec!["cli/**".to_string()], &["cfg/**".to_string()]);
        assert_eq!(out, vec!["cli/**".to_string()]);
    }

    // --- resolve_probe_env ------------------------------------------------

    #[test]
    fn resolve_probe_env_parses_config_when_cli_empty() {
        let out = resolve_probe_env(vec![], &["A=1".to_string()]).unwrap();
        assert_eq!(out, vec![("A".to_string(), "1".to_string())]);
    }

    #[test]
    fn resolve_probe_env_keeps_cli_and_ignores_config() {
        let cli = vec![("X".to_string(), "y".to_string())];
        let out = resolve_probe_env(cli.clone(), &["A=1".to_string()]).unwrap();
        assert_eq!(out, cli);
    }

    // --- resolve_operators ------------------------------------------------

    #[test]
    fn resolve_operators_uses_config_when_cli_empty() {
        let out = resolve_operators(vec![], Some(&vec!["swap_boolean".to_string()]), None).unwrap();
        assert_eq!(out, vec![core::OperatorName::SwapBoolean]);
    }

    #[test]
    fn resolve_operators_keeps_cli_and_ignores_config() {
        let out = resolve_operators(
            vec![core::OperatorName::RemoveNot],
            Some(&vec!["swap_boolean".to_string()]),
            Some(&vec!["comparison".to_string()]),
        )
        .unwrap();
        assert_eq!(out, vec![core::OperatorName::RemoveNot]);
    }

    #[test]
    fn resolve_operators_dedupes_category_operators() {
        let cat_ops = core::OperatorCategory::Comparison.operators();
        assert!(cat_ops.len() >= 2, "test needs a multi-operator category");
        // The first operator is supplied explicitly *and* by the category, so
        // the de-dup guard must keep it exactly once.
        let out = resolve_operators(
            vec![],
            Some(&vec![cat_ops[0].as_str().to_string()]),
            Some(&vec!["comparison".to_string()]),
        )
        .unwrap();
        assert_eq!(out, cat_ops);
    }

    // --- resolve_exclude_operators ----------------------------------------

    #[test]
    fn resolve_exclude_operators_uses_config_when_cli_empty() {
        let out = resolve_exclude_operators(vec![], &["swap_boolean".to_string()], &[]).unwrap();
        assert_eq!(out, vec![core::OperatorName::SwapBoolean]);
    }

    #[test]
    fn resolve_exclude_operators_keeps_cli_and_ignores_config() {
        let out = resolve_exclude_operators(
            vec![core::OperatorName::RemoveNot],
            &["swap_boolean".to_string()],
            &["comparison".to_string()],
        )
        .unwrap();
        assert_eq!(out, vec![core::OperatorName::RemoveNot]);
    }

    #[test]
    fn resolve_exclude_operators_dedupes_category_operators() {
        let cat_ops = core::OperatorCategory::Comparison.operators();
        assert!(cat_ops.len() >= 2, "test needs a multi-operator category");
        let out = resolve_exclude_operators(
            vec![],
            &[cat_ops[0].as_str().to_string()],
            &["comparison".to_string()],
        )
        .unwrap();
        assert_eq!(out, cat_ops);
    }

    // --- num_workers ------------------------------------------------------

    #[test]
    fn num_workers_uses_jobs_without_per_worker_dirs() {
        assert_eq!(num_workers(4, &[]), 4);
        assert_eq!(num_workers(0, &[]), 1); // floored at 1
    }

    #[test]
    fn num_workers_counts_per_worker_dirs() {
        let dirs = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];
        assert_eq!(num_workers(1, &dirs), 3);
    }

    // --- worker_probe_env_dirs --------------------------------------------

    fn templates(pairs: &[(&str, &str)]) -> Vec<execution::ProbeEnvTemplate> {
        pairs
            .iter()
            .map(|(k, v)| execution::ProbeEnvTemplate::parse((*k).to_string(), v))
            .collect()
    }

    #[test]
    fn worker_probe_env_dirs_expands_one_per_worker() {
        let env = templates(&[("CACHE", "dir/{worker}")]);
        let dirs = worker_probe_env_dirs(&env, 2);
        assert_eq!(dirs, vec![PathBuf::from("dir/0"), PathBuf::from("dir/1")]);
    }

    #[test]
    fn worker_probe_env_dirs_skips_values_without_template() {
        let env = templates(&[("CACHE", "dir/static")]);
        assert!(worker_probe_env_dirs(&env, 2).is_empty());
    }

    #[test]
    fn worker_probe_env_dirs_skips_non_path_values() {
        let env = templates(&[("N", "n{worker}")]);
        // "n0"/"n1" are not path-like, so nothing is created.
        assert!(worker_probe_env_dirs(&env, 2).is_empty());
    }

    // --- worker_build_cache_arg -------------------------------------------

    #[test]
    fn worker_build_cache_arg_none_when_empty() {
        assert!(worker_build_cache_arg(&[]).is_none());
    }

    #[test]
    fn worker_build_cache_arg_some_when_present() {
        let dirs = vec![PathBuf::from("a")];
        assert_eq!(worker_build_cache_arg(&dirs), Some(dirs.as_slice()));
    }

    // --- warmup_target ----------------------------------------------------

    #[test]
    fn warmup_target_prefers_workers() {
        let dirs = vec![PathBuf::from("a")];
        let shared = std::path::Path::new("/shared");
        assert_eq!(warmup_target(&dirs, Some(shared)), WarmupTarget::Workers);
    }

    #[test]
    fn warmup_target_falls_back_to_shared() {
        let shared = std::path::Path::new("/shared");
        assert_eq!(
            warmup_target(&[], Some(shared)),
            WarmupTarget::Shared(shared)
        );
    }

    #[test]
    fn warmup_target_nothing_when_no_dirs() {
        assert_eq!(warmup_target(&[], None), WarmupTarget::Nothing);
    }

    // --- warmup_status_to_result ------------------------------------------

    #[cfg(unix)]
    #[test]
    fn warmup_status_ok_on_success() {
        use std::os::unix::process::ExitStatusExt;
        let status = std::process::ExitStatus::from_raw(0);
        assert!(warmup_status_to_result(status).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn warmup_status_err_on_failure() {
        use std::os::unix::process::ExitStatusExt;
        let status = std::process::ExitStatus::from_raw(256); // exit code 1
        assert!(warmup_status_to_result(status).is_err());
    }

    // --- progress_enabled -------------------------------------------------

    #[test]
    fn progress_enabled_truth_table() {
        assert!(progress_enabled(false, true)); // not quiet, mode allows
        assert!(!progress_enabled(true, true)); // quiet suppresses
        assert!(!progress_enabled(false, false)); // mode disallows
        assert!(!progress_enabled(true, false));
    }
}
