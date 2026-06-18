mod core;
mod lang;
mod crap;
mod mutate;
mod runner;
mod scheduler;
mod report;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};

const DEFAULT_EXCLUDES: &[&str] = &[
    "target/**",
    ".ooze/**",
    ".git/**",
];

fn resolve_excludes(user: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect();
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
    #[command(about = "Apply a mutation in a copy-on-write workspace and print the diff")]
    ApplyMutant {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        id: String,
    },
    #[command(about = "Run a batch of mutations sequentially and produce a summary report")]
    TestMutants {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        lcov: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = scheduler::MutationStrategy::Discovery)]
        strategy: scheduler::MutationStrategy,

        #[arg(long)]
        limit: Option<usize>,

        #[arg(long, default_value_t = 1)]
        jobs: usize,

        #[arg(long)]
        timeout_seconds: Option<u64>,

        #[arg(long, help = "Shared CARGO_TARGET_DIR for probe runs (default: <cache_dir>/cargo-target)")]
        cargo_target_dir: Option<PathBuf>,

        #[arg(long, help = "Disable the shared CARGO_TARGET_DIR for probes")]
        no_shared_target: bool,

        #[arg(long, value_enum, default_value_t = WorkspaceBackendArg::Auto)]
        workspace_backend: WorkspaceBackendArg,

        #[arg(long, default_value = ".ooze/cache")]
        cache_dir: PathBuf,

        #[arg(long, default_value = ".ooze/runs")]
        runs_dir: PathBuf,

        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,

        #[arg(last = true)]
        probe: Vec<String>,
    },
    #[command(about = "Warm up the shared Cargo cache before running mutants")]
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
            let excludes = resolve_excludes(&exclude);
            let functions = lang::scan_directory_with_excludes(&path, &excludes)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
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
            path,
            lcov,
            strategy,
            limit,
            jobs,
            timeout_seconds,
            cargo_target_dir,
            no_shared_target,
            workspace_backend,
            cache_dir,
            runs_dir,
            exclude,
            probe,
        } => {
            let excludes = resolve_excludes(&exclude);
            let functions = lang::scan_directory_with_excludes(&path, &excludes)?;
            let languages = lang::supported_languages();
            let candidates = mutate::discover_mutants(&functions, &languages)?;

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

            let target_dir: Option<PathBuf> = if no_shared_target {
                None
            } else {
                Some(
                    cargo_target_dir
                        .unwrap_or_else(|| runner::default_cargo_target_dir(&cache_dir)),
                )
            };

            if let Some(dir) = target_dir.as_ref() {
                std::fs::create_dir_all(dir).with_context(|| {
                    format!("creating cargo target dir {}", dir.display())
                })?;
            }

            let cfg = runner::BatchConfig {
                backend: workspace_backend.resolve(),
                timeout,
                cargo_target_dir: target_dir.as_deref(),
                runs_dir: &runs_dir,
            };

            let report = runner::run_mutants_parallel(
                &repo_root,
                candidates,
                &probe,
                jobs,
                cfg,
            )?;

            println!("{}", serde_json::to_string_pretty(&report)?);
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
            let target_dir = runner::default_cargo_target_dir(&cache_dir);
            let status = runner::warmup(&repo_root, &probe, Some(&target_dir))?;
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
            let outcome = workspace.run_probe(applied, &probe, None, None)?;

            println!("{}", serde_json::to_string_pretty(&outcome)?);
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
