mod core;
mod lang;
mod crap;
mod mutate;
mod runner;
mod skip;
mod scheduler;
mod report;
mod config;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};

const DEFAULT_EXCLUDES: &[&str] = &[
    "target/**",
    ".ooze/**",
    ".git/**",
    "node_modules/**",
    "vendor/**",
    "__pycache__/**",
    ".gradle/**",
];

fn read_gitignore_patterns(root: &std::path::Path) -> Vec<String> {
    let path = root.join(".gitignore");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches('/').to_string())
        .collect()
}

fn parse_operator(s: &str) -> Result<core::OperatorName, String> {
    core::OperatorName::parse(s).ok_or_else(|| {
        let names: Vec<&str> = core::OperatorName::ALL.iter().map(|o| o.as_str()).collect();
        format!("unknown operator {s:?}; known: {}", names.join(", "))
    })
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got {s:?}"))?;
    if k.is_empty() {
        return Err(format!("empty key in {s:?}"));
    }
    Ok((k.to_string(), v.to_string()))
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

fn resolve_excludes(root: &std::path::Path, user: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect();
    out.extend(read_gitignore_patterns(root));
    out.extend(user.iter().cloned());
    out
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum WorkspaceBackendArg {
    Copy,
    Overlay,
    Auto,
}

impl WorkspaceBackendArg {
    fn resolve(self) -> runner::WorkspaceBackend {
        match self {
            WorkspaceBackendArg::Copy => runner::WorkspaceBackend::Copy,
            WorkspaceBackendArg::Overlay => runner::WorkspaceBackend::Overlay,
            WorkspaceBackendArg::Auto => {
                if runner::overlay::overlay_available() {
                    runner::WorkspaceBackend::Overlay
                } else {
                    runner::WorkspaceBackend::Copy
                }
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "ooze")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn parse_strategy_str(s: &str) -> anyhow::Result<scheduler::MutationStrategy> {
    <scheduler::MutationStrategy as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid strategy {s:?}: {e}"))
}

fn parse_workspace_backend_str(s: &str) -> anyhow::Result<WorkspaceBackendArg> {
    <WorkspaceBackendArg as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid workspace_backend {s:?}: {e}"))
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Scan source files and extract function spans")]
    Scan {
        #[arg(short, long, default_value = ".")]
        path: String,
        #[arg(long, default_value = "json")]
        format: String,
    },
    #[command(about = "Score functions by CRAP formula")]
    Crap {
        #[arg(short, long, default_value = ".")]
        path: String,
        #[arg(long)]
        lcov: Option<PathBuf>,
        #[arg(long, default_value = "json")]
        format: String,
    },
    #[command(about = "Discover mutation candidates")]
    Mutants {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,
    },
    #[command(about = "List available mutation operators and their metadata")]
    Operators {
        #[arg(long, default_value = "json")]
        format: String,
    },
    #[command(about = "Plan a mutation run without executing probes: shows selection, scores, and applied excludes")]
    PlanMutants {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        lcov: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = scheduler::MutationStrategy::Discovery)]
        strategy: scheduler::MutationStrategy,

        #[arg(long)]
        limit: Option<usize>,

        #[arg(long, default_value = "json")]
        format: String,

        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,

        #[arg(long, value_delimiter = ',', value_parser = parse_operator, help = "Restrict to these operators (comma-separated).")]
        operators: Vec<core::OperatorName>,

        #[arg(long = "exclude-operators", value_delimiter = ',', value_parser = parse_operator, help = "Drop these operators (comma-separated).")]
        exclude_operators: Vec<core::OperatorName>,

        #[arg(long, help = "Disable static skip rules (test files, assertion/panic macros, generated files).")]
        no_static_skips: bool,

        #[arg(long, help = "Include the full list of skipped candidates in the output.")]
        show_skipped: bool,
    },
    #[command(about = "Apply a mutation in a copy-on-write workspace and print the diff")]
    ApplyMutant {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        id: String,
    },
    #[command(about = "Run a batch of mutations sequentially and produce a summary report")]
    TestMutants {
        #[arg(long, help = "Path to ooze.toml config (default: ./ooze.toml if present).")]
        config: Option<PathBuf>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        lcov: Option<PathBuf>,

        #[arg(long, value_enum)]
        strategy: Option<scheduler::MutationStrategy>,

        #[arg(long)]
        limit: Option<usize>,

        #[arg(long)]
        jobs: Option<usize>,

        #[arg(long)]
        timeout_seconds: Option<u64>,

        #[arg(long, help = "Shared build cache dir for probe runs (default: <cache_dir>/build-cache). Reference it as {build_cache} in --probe-env.")]
        build_cache_dir: Option<PathBuf>,

        #[arg(long, help = "Give each worker its own build-cache-job-{i} dir instead of a shared one")]
        per_worker_cache: bool,

        #[arg(long, help = "Pre-build the probe in each worker target dir before running mutants")]
        warmup: bool,

        #[arg(long, value_enum)]
        workspace_backend: Option<WorkspaceBackendArg>,

        #[arg(long)]
        cache_dir: Option<PathBuf>,

        #[arg(long)]
        runs_dir: Option<PathBuf>,

        #[arg(long, help = "Report format: json, human, agent-tasks-json, agent-tasks-markdown, github-annotations, sarif")]
        format: Option<String>,

        #[arg(long, help = "Write report to a file instead of stdout.")]
        output: Option<PathBuf>,

        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,

        #[arg(long = "probe-env", value_parser = parse_key_val, help = "KEY=VALUE env var to set on probe (and warmup). {worker} in VALUE expands to the worker index. Repeatable.")]
        probe_env: Vec<(String, String)>,

        #[arg(long, value_delimiter = ',', value_parser = parse_operator, help = "Restrict to these operators (comma-separated).")]
        operators: Vec<core::OperatorName>,

        #[arg(long = "exclude-operators", value_delimiter = ',', value_parser = parse_operator, help = "Drop these operators (comma-separated).")]
        exclude_operators: Vec<core::OperatorName>,

        #[arg(long, help = "Disable static skip rules (test files, assertion/panic macros, generated files).")]
        no_static_skips: bool,

        #[arg(long, help = "Lines of source context around each survived mutant (0 disables).")]
        context_lines: Option<usize>,

        #[arg(long, help = "Run the probe once on unmodified code first; abort if it fails or times out.")]
        preflight: bool,

        #[arg(long, help = "Exit 0 even if survivors are found (timeouts/errors still surface).")]
        no_fail_on_survivors: bool,

        #[arg(long, help = "Treat timeout/error outcomes as non-fatal for exit code purposes.")]
        allow_incomplete: bool,

        #[arg(last = true)]
        probe: Vec<String>,
    },
    #[command(about = "Write a starter ooze.toml in the current directory")]
    InitConfig {
        #[arg(long, default_value = "ooze.toml")]
        path: PathBuf,

        #[arg(long, help = "Overwrite an existing config file")]
        force: bool,

        #[arg(long, help = "Language preset: rust, go, python, node, java-gradle, java-maven, ruby. Prompted interactively if omitted.")]
        language: Option<String>,
    },
    #[command(about = "Warm up the shared build cache before running mutants")]
    Warmup {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, default_value = ".ooze/cache")]
        cache_dir: PathBuf,

        #[arg(last = true)]
        probe: Vec<String>,
    },
    #[command(about = "Apply a mutation in a workspace, run a probe, and classify the result")]
    TestMutant {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        id: String,

        #[arg(last = true)]
        probe: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { path, format } => {
            let spans = lang::scan_directory(std::path::Path::new(&path))?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&spans)?);
            }
        }
        Commands::Mutants { path, format, exclude } => {
            let excludes = resolve_excludes(&path, &exclude);
            let functions = lang::scan_directory_with_excludes(&path, &excludes)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
            }
        }
        Commands::Operators { format } => {
            let infos: Vec<_> = core::OperatorName::ALL.iter().map(|o| o.info()).collect();
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&infos)?);
            } else {
                for info in &infos {
                    println!(
                        "{:<18} [{}] default_enabled={}\n  {}\n  hint: {}\n",
                        info.name, info.category, info.default_enabled, info.description, info.test_hint
                    );
                }
            }
        }
        Commands::PlanMutants {
            path,
            lcov,
            strategy,
            limit,
            format,
            exclude,
            operators,
            exclude_operators,
            no_static_skips,
            show_skipped,
        } => {
            let excludes = resolve_excludes(&path, &exclude);
            let functions = lang::scan_directory_with_excludes(&path, &excludes)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;
            let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);
            let candidates = filter.apply(candidates);
            let total_candidates = candidates.len();
            let (candidates, skipped_candidates) = if no_static_skips {
                (candidates, Vec::new())
            } else {
                skip::partition(candidates)
            };
            let skipped_count = skipped_candidates.len();

            let crap_entries = if let Some(lcov_path) = lcov.as_ref() {
                let coverage = crap::coverage::parse_lcov(lcov_path)?;
                crap::score_with_coverage(functions, coverage)
            } else {
                crap::score_without_coverage(functions)
            };

            let mut ordered = scheduler::order(strategy, candidates, &crap_entries);
            if let Some(limit) = limit {
                ordered.truncate(limit);
            }

            #[derive(serde::Serialize)]
            struct PlannedCandidate {
                #[serde(flatten)]
                candidate: core::MutationCandidate,
                #[serde(flatten)]
                selection: scheduler::SelectionExplanation,
            }

            let planned: Vec<PlannedCandidate> = ordered
                .into_iter()
                .map(|c| {
                    let selection = scheduler::explain(strategy, &c, &crap_entries);
                    PlannedCandidate { candidate: c, selection }
                })
                .collect();

            #[derive(serde::Serialize)]
            struct Plan {
                total_candidates: usize,
                skipped: usize,
                selected: usize,
                strategy: String,
                excluded_patterns: Vec<String>,
                operator_filter: mutate::OperatorFilterReport,
                candidates: Vec<PlannedCandidate>,
                #[serde(skip_serializing_if = "Option::is_none")]
                skipped_candidates: Option<Vec<skip::SkippedCandidate>>,
            }

            let plan = Plan {
                total_candidates,
                skipped: skipped_count,
                selected: planned.len(),
                strategy: format!("{strategy:?}").to_lowercase(),
                excluded_patterns: excludes,
                operator_filter: (&filter).into(),
                candidates: planned,
                skipped_candidates: if show_skipped {
                    Some(skipped_candidates)
                } else {
                    None
                },
            };

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        Commands::ApplyMutant { path, id } => {
            let repo_root = path;

            let functions = lang::scan_directory(&repo_root)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;

            let Some(candidate) = candidates.into_iter().find(|c| c.id == id) else {
                anyhow::bail!("no mutation candidate found with id {id:?}");
            };

            let workspace = runner::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;

            println!("{}", applied.diff);
        }
        Commands::TestMutants {
            config: config_path,
            path,
            lcov,
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
            exclude,
            probe_env,
            operators,
            exclude_operators,
            no_static_skips,
            context_lines,
            preflight,
            no_fail_on_survivors,
            allow_incomplete,
            probe,
        } => {
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
            let timeout_seconds = timeout_seconds.or(cfg.runner.timeout_seconds);
            let build_cache_dir = build_cache_dir.or(cfg.runner.build_cache_dir.clone());
            let per_worker_cache =
                per_worker_cache || cfg.runner.per_worker_cache == Some(true);
            let warmup = warmup || cfg.runner.warmup == Some(true);
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
            let format = format
                .or(cfg.report.format.clone())
                .unwrap_or_else(|| "json".to_string());
            let output = output.or(cfg.report.output.clone());
            let no_static_skips = no_static_skips || cfg.mutation.static_skips == Some(false);
            let context_lines = context_lines.or(cfg.mutation.context_lines).unwrap_or(3);
            let preflight = preflight || cfg.runner.preflight == Some(true);
            let no_fail_on_survivors =
                no_fail_on_survivors || cfg.report.fail_on_survivors == Some(false);
            let allow_incomplete = allow_incomplete || cfg.report.allow_incomplete == Some(true);
            let lcov = lcov.or(cfg.mutation.lcov.clone());

            let mut exclude = exclude;
            if exclude.is_empty() {
                exclude.extend(cfg.scope.exclude.iter().cloned());
            }

            let mut probe_env = probe_env;
            if probe_env.is_empty() {
                for entry in &cfg.probe.env {
                    probe_env.push(parse_key_val(entry).map_err(|e| anyhow::anyhow!(e))?);
                }
            }

            let mut operators = operators;
            if operators.is_empty() {
                if let Some(ops) = cfg.mutation.operators.as_ref() {
                    for s in ops {
                        operators.push(
                            core::OperatorName::parse(s).ok_or_else(|| {
                                anyhow::anyhow!(
                                    "unknown operator {s:?} in [mutation].operators"
                                )
                            })?,
                        );
                    }
                }
            }

            let mut exclude_operators = exclude_operators;
            if exclude_operators.is_empty() {
                for s in &cfg.mutation.exclude_operators {
                    exclude_operators.push(
                        core::OperatorName::parse(s).ok_or_else(|| {
                            anyhow::anyhow!(
                                "unknown operator {s:?} in [mutation].exclude_operators"
                            )
                        })?,
                    );
                }
            }

            let mut probe = probe;
            if probe.is_empty() {
                if let Some(cmd) = cfg.probe.command.as_ref() {
                    probe = cmd.clone();
                } else {
                    anyhow::bail!(
                        "missing probe command; pass one after `--` or set [probe].command in ooze.toml"
                    );
                }
            }

            let excludes = resolve_excludes(&path, &exclude);
            let functions = lang::scan_directory_with_excludes(&path, &excludes)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;
            let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);
            let candidates = filter.apply(candidates);
            let candidates = if no_static_skips {
                candidates
            } else {
                let (kept, _) = skip::partition(candidates);
                kept
            };

            let crap_entries = if let Some(lcov_path) = lcov.as_ref() {
                let coverage = crap::coverage::parse_lcov(lcov_path)?;
                crap::score_with_coverage(functions, coverage)
            } else {
                crap::score_without_coverage(functions)
            };

            let mut candidates = scheduler::order(strategy, candidates, &crap_entries);

            if let Some(limit) = limit {
                candidates.truncate(limit);
            }

            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;

            let timeout = timeout_seconds.map(std::time::Duration::from_secs);

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
            std::fs::create_dir_all(&cache_dir).with_context(|| {
                format!("creating cache dir {}", cache_dir.display())
            })?;
            std::fs::create_dir_all(&runs_dir).with_context(|| {
                format!("creating runs dir {}", runs_dir.display())
            })?;

            let (target_dir, worker_build_cache_dirs): (Option<PathBuf>, Vec<PathBuf>) =
                if per_worker_cache {
                    if jobs > 1 {
                        let dirs: Vec<PathBuf> = (0..jobs)
                            .map(|i| cache_dir.join(format!("build-cache-job-{i}")))
                            .collect();
                        for d in &dirs {
                            std::fs::create_dir_all(d).with_context(|| {
                                format!("creating worker build cache dir {}", d.display())
                            })?;
                        }
                        (None, dirs)
                    } else {
                        (None, Vec::new())
                    }
                } else {
                    let dir = build_cache_dir
                        .unwrap_or_else(|| runner::default_build_cache_dir(&cache_dir));
                    std::fs::create_dir_all(&dir).with_context(|| {
                        format!("creating build cache dir {}", dir.display())
                    })?;
                    (Some(dir), Vec::new())
                };

            let num_workers = if !worker_build_cache_dirs.is_empty() {
                worker_build_cache_dirs.len()
            } else {
                jobs.max(1)
            };
            for (_, v) in &probe_env {
                if !v.contains("{worker}") {
                    continue;
                }
                for i in 0..num_workers {
                    let resolved = v.replace("{worker}", &i.to_string());
                    let p = std::path::Path::new(&resolved);
                    let looks_like_path = resolved.contains('/')
                        || resolved.starts_with('.')
                        || p.is_absolute();
                    if looks_like_path {
                        std::fs::create_dir_all(p).with_context(|| {
                            format!("creating probe-env directory {}", p.display())
                        })?;
                    }
                }
            }

            if preflight {
                let preflight_build_cache = target_dir
                    .as_deref()
                    .or_else(|| worker_build_cache_dirs.first().map(|p| p.as_path()));
                let preflight_envs: Vec<(String, String)> = probe_env
                    .iter()
                    .map(|(k, v)| {
                        let v = v.replace("{worker}", "0");
                        let v = if let Some(dir) = preflight_build_cache {
                            v.replace("{build_cache}", &dir.to_string_lossy())
                        } else {
                            v
                        };
                        (k.clone(), v)
                    })
                    .collect();
                let outcome = runner::preflight(
                    &repo_root,
                    &probe,
                    timeout,
                    &preflight_envs,
                )?;
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
                    if format == "human" {
                        eprintln!("Preflight failed.\n");
                        eprintln!("{}\n", msg);
                        eprintln!("Command: {}", probe.join(" "));
                        if let Some(code) = payload.exit_code {
                            eprintln!("Exit code: {}", code);
                        }
                    } else {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    }
                    std::process::exit(report::OozeExitCode::PreflightFailed.code());
                }
            }

            if warmup {
                if !worker_build_cache_dirs.is_empty() {
                    eprintln!(
                        "warming up {} worker build cache dirs in parallel...",
                        worker_build_cache_dirs.len()
                    );
                    runner::warmup_workers(
                        &repo_root,
                        &probe,
                        &worker_build_cache_dirs,
                        jobs,
                        &probe_env,
                    )?;
                } else if let Some(dir) = target_dir.as_deref() {
                    eprintln!("warming up shared build cache dir...");
                    let extra: Vec<(String, String)> = probe_env
                        .iter()
                        .map(|(k, v)| {
                            let v = v.replace("{worker}", "0");
                            let v = v.replace("{build_cache}", &dir.to_string_lossy());
                            (k.clone(), v)
                        })
                        .collect();
                    let status = runner::warmup(&repo_root, &probe, Some(dir), &extra)?;
                    if !status.success() {
                        anyhow::bail!("warmup command failed with status {status}");
                    }
                }
            }

            let cfg = runner::BatchConfig {
                backend: workspace_backend.resolve(),
                timeout,
                build_cache_dir: target_dir.as_deref(),
                worker_build_cache_dirs: if worker_build_cache_dirs.is_empty() {
                    None
                } else {
                    Some(&worker_build_cache_dirs)
                },
                probe_env_templates: &probe_env,
                runs_dir: &runs_dir,
            };

            let raw_report = runner::run_mutants_parallel(
                &repo_root,
                candidates,
                &probe,
                jobs,
                cfg,
            )?;

            let enriched = report::enrich(raw_report, &crap_entries, &repo_root, context_lines);

            let text = match format.as_str() {
                "human" => report::human(&enriched),
                "agent-tasks-json" => {
                    let tasks = report::agent_tasks(&enriched);
                    let mut s = serde_json::to_string_pretty(&tasks)?;
                    s.push('\n');
                    s
                }
                "agent-tasks-markdown" => {
                    let tasks = report::agent_tasks(&enriched);
                    report::agent_tasks_markdown(&tasks)
                }
                "github-annotations" => report::github_annotations(&enriched),
                "sarif" => {
                    let log = report::sarif(&enriched);
                    let mut s = serde_json::to_string_pretty(&log)?;
                    s.push('\n');
                    s
                }
                _ => {
                    let mut s = serde_json::to_string_pretty(&enriched)?;
                    s.push('\n');
                    s
                }
            };
            match output.as_deref() {
                Some(path) => std::fs::write(path, &text)
                    .with_context(|| format!("writing report to {}", path.display()))?,
                None => print!("{}", text),
            }

            let exit = report::exit_code_for_report(
                &enriched,
                no_fail_on_survivors,
                allow_incomplete,
            );
            std::process::exit(exit.code());
        }
        Commands::Warmup {
            path,
            cache_dir,
            probe,
        } => {
            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;
            let cache_dir = if cache_dir.is_absolute() {
                cache_dir
            } else {
                repo_root.join(&cache_dir)
            };
            let target_dir = runner::default_build_cache_dir(&cache_dir);
            let status = runner::warmup(&repo_root, &probe, Some(&target_dir), &[])?;
            if !status.success() {
                anyhow::bail!("warmup command failed with status {status}");
            }
        }
        Commands::TestMutant { path, id, probe } => {
            let functions = lang::scan_directory(&path)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;

            let Some(candidate) = candidates.into_iter().find(|c| c.id == id) else {
                anyhow::bail!("no mutation candidate found with id {id:?}");
            };

            let repo_root = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalizing {}", path.display()))?;

            let workspace = runner::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;
            let outcome = workspace.run_probe(applied, &probe, None)?;

            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Commands::InitConfig { path, force, language } => {
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
            format,
        } => {
            let functions = lang::scan_directory(std::path::Path::new(&path))?;
            let entries = if let Some(lcov_path) = lcov.as_ref() {
                let coverage = crap::coverage::parse_lcov(lcov_path)?;
                crap::score_with_coverage(functions, coverage)
            } else {
                crap::score_without_coverage(functions)
            };
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            }
        }
    }
    Ok(())
}
