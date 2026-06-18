mod core;
mod lang;
mod crap;
mod mutate;
mod runner;
mod report;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

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
    },
    #[command(about = "Apply a mutation in a copy-on-write workspace and print the diff")]
    ApplyMutant {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        id: String,
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
        Commands::Mutants { path, format } => {
            let functions = lang::scan_directory(&path)?;
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
            let outcome = workspace.run_probe(applied, &probe)?;

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
