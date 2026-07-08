//! Command-line surface: the Clap argument types, their value parsers, and the
//! small CLI-only enums. Resolution of these into runnable settings lives in
//! `crate::app`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{core, report, scheduler, workspace};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ProgressMode {
    Auto,
    Always,
    Never,
}

impl ProgressMode {
    pub(crate) fn resolve(self) -> bool {
        match self {
            ProgressMode::Always => true,
            ProgressMode::Never => false,
            ProgressMode::Auto => {
                use std::io::IsTerminal;
                let ci = std::env::var_os("CI").is_some();
                !ci && std::io::stderr().is_terminal()
            }
        }
    }
}

const COVERAGE_HELP: &str = "Coverage report as `format:path` or a bare path to auto-detect. \
Formats: lcov, cobertura, jacoco, go-cover. Repeatable; multiple reports are merged. \
E.g. --coverage cobertura:coverage.xml";

fn parse_operator(s: &str) -> Result<core::OperatorName, String> {
    core::OperatorName::parse(s).ok_or_else(|| {
        let names: Vec<&str> = core::OperatorName::ALL
            .iter()
            .copied()
            .map(core::OperatorName::as_str)
            .collect();
        format!("unknown operator {s:?}; known: {}", names.join(", "))
    })
}

pub(crate) fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got {s:?}"))?;
    if k.is_empty() {
        return Err(format!("empty key in {s:?}"));
    }
    Ok((k.to_string(), v.to_string()))
}

/// Output shape for the simple introspection commands (scan, crap, mutants,
/// operators, plan-mutants, doctor): either machine JSON or a human rendering.
/// The richer `test-mutants` report uses `report::ReportFormat` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lower")]
pub(crate) enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub(crate) fn is_json(self) -> bool {
        matches!(self, OutputFormat::Json)
    }
}

/// The `--preset` flag value. Purely the CLI spelling of a preset; what a
/// preset actually does at runtime lives in [`crate::preset::Preset`], which
/// this converts into via `From`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum PresetArg {
    Rust,
    Go,
    Node,
    Python,
    /// Spelled `csharp` on the CLI (`c#` is awkward in shells).
    #[value(name = "csharp")]
    CSharp,
}

impl From<PresetArg> for crate::preset::Preset {
    fn from(value: PresetArg) -> Self {
        match value {
            PresetArg::Rust => crate::preset::Preset::Rust,
            PresetArg::Go => crate::preset::Preset::Go,
            PresetArg::Node => crate::preset::Preset::Node,
            PresetArg::Python => crate::preset::Preset::Python,
            PresetArg::CSharp => crate::preset::Preset::CSharp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum WorkspaceBackendArg {
    Copy,
    Overlay,
    Worktree,
    Auto,
}

impl WorkspaceBackendArg {
    /// The backend's CLI value, e.g. for `workspace_backend=worktree` in
    /// preset fill descriptions.
    pub(crate) fn cli_name(self) -> &'static str {
        match self {
            WorkspaceBackendArg::Copy => "copy",
            WorkspaceBackendArg::Overlay => "overlay",
            WorkspaceBackendArg::Worktree => "worktree",
            WorkspaceBackendArg::Auto => "auto",
        }
    }

    /// `auto` prefers the rootless worktree backend when `repo_root` is inside
    /// a Git repository and falls back to copy otherwise. Overlay stays
    /// explicit: its mount needs root, so it should never win by default.
    pub(crate) fn resolve(self, repo_root: &std::path::Path) -> workspace::WorkspaceBackend {
        match self {
            WorkspaceBackendArg::Copy => workspace::WorkspaceBackend::Copy,
            WorkspaceBackendArg::Overlay => workspace::WorkspaceBackend::Overlay,
            WorkspaceBackendArg::Worktree => workspace::WorkspaceBackend::Worktree,
            WorkspaceBackendArg::Auto => {
                if workspace::worktree::is_git_repo(repo_root) {
                    workspace::WorkspaceBackend::Worktree
                } else {
                    workspace::WorkspaceBackend::Copy
                }
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "ooze")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    #[command(about = "Scan source files and extract function spans")]
    Scan {
        #[arg(short, long, default_value = ".")]
        path: String,
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(about = "Score functions by CRAP formula")]
    Crap {
        #[arg(short, long, default_value = ".")]
        path: String,
        #[arg(long, help = "DEPRECATED: use --coverage. Path to an LCOV tracefile.")]
        lcov: Option<PathBuf>,
        #[arg(long, value_name = "SPEC", help = COVERAGE_HELP)]
        coverage: Vec<String>,
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(about = "Discover mutation candidates")]
    Mutants {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git."
        )]
        exclude: Vec<String>,
    },
    #[command(about = "List available mutation operators and their metadata")]
    Operators {
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(
        about = "List supported languages and how far their support goes (scan-only vs mutation)"
    )]
    Languages {
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(
        about = "Plan a mutation run without executing probes: shows selection, scores, and applied excludes"
    )]
    PlanMutants {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "DEPRECATED: use --coverage. Path to an LCOV tracefile.")]
        lcov: Option<PathBuf>,

        #[arg(long, value_name = "SPEC", help = COVERAGE_HELP)]
        coverage: Vec<String>,

        #[arg(long, value_enum, default_value_t = scheduler::MutationStrategy::Discovery)]
        strategy: scheduler::MutationStrategy,

        #[arg(long)]
        limit: Option<usize>,

        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,

        #[arg(
            long,
            value_delimiter = ',',
            help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git."
        )]
        exclude: Vec<String>,

        #[arg(
            long,
            value_name = "BASE",
            help = "Only mutate files changed versus BASE (e.g. `main`): git diff BASE...HEAD plus uncommitted and untracked changes."
        )]
        changed_only: Option<String>,

        #[arg(long, value_delimiter = ',', value_parser = parse_operator, help = "Restrict to these operators (comma-separated).")]
        operators: Vec<core::OperatorName>,

        #[arg(long = "exclude-operators", value_delimiter = ',', value_parser = parse_operator, help = "Drop these operators (comma-separated).")]
        exclude_operators: Vec<core::OperatorName>,

        #[arg(
            long,
            help = "Disable static skip rules (test files, assertion/panic macros, generated files)."
        )]
        no_static_skips: bool,

        #[arg(
            long,
            help = "Include the full list of skipped candidates in the output."
        )]
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
    TestMutants(Box<TestMutantsArgs>),
    #[command(about = "Write a starter ooze.toml in the current directory")]
    InitConfig {
        #[arg(long, default_value = "ooze.toml")]
        path: PathBuf,

        #[arg(long, help = "Overwrite an existing config file")]
        force: bool,

        #[arg(
            long,
            help = "Language preset: rust, go, python, node, java-gradle, java-maven, ruby. Prompted interactively if omitted."
        )]
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
    #[command(about = "Diagnose repo, config, and runtime preconditions for ooze")]
    Doctor {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(
            long,
            value_enum,
            default_value = "human",
            help = "Output format: human or json"
        )]
        format: OutputFormat,

        #[arg(
            long,
            help = "Show operator support for the detected project languages"
        )]
        operators: bool,
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

/// Args for `TestMutants`. Extracted into its own struct (and boxed in the
/// `Commands` variant) so this large, rarely-constructed variant doesn't bloat
/// every `Commands` value.
#[derive(clap::Args)]
pub(crate) struct TestMutantsArgs {
    #[arg(
        long,
        help = "Path to ooze.toml config (default: <path>/ooze.toml if present)."
    )]
    pub(crate) config: Option<PathBuf>,

    #[arg(long, default_value = ".")]
    pub(crate) path: PathBuf,

    #[arg(long, help = "DEPRECATED: use --coverage. Path to an LCOV tracefile.")]
    pub(crate) lcov: Option<PathBuf>,

    #[arg(long, value_name = "SPEC", help = COVERAGE_HELP)]
    pub(crate) coverage: Vec<String>,

    #[arg(long, value_enum)]
    pub(crate) strategy: Option<scheduler::MutationStrategy>,

    #[arg(long)]
    pub(crate) limit: Option<usize>,

    #[arg(long)]
    pub(crate) jobs: Option<usize>,

    #[arg(long)]
    pub(crate) timeout_seconds: Option<u64>,

    #[arg(
        long,
        help = "Shared build cache dir for probe runs (default: <cache_dir>/build-cache). Reference it as {build_cache} in --probe-env."
    )]
    pub(crate) build_cache_dir: Option<PathBuf>,

    #[arg(
        long,
        help = "Give each worker its own build-cache-job-{i} dir instead of a shared one"
    )]
    pub(crate) per_worker_cache: bool,

    #[arg(
        long,
        help = "Pre-build the probe in each worker target dir before running mutants"
    )]
    pub(crate) warmup: bool,

    #[arg(long, value_enum)]
    pub(crate) workspace_backend: Option<WorkspaceBackendArg>,

    #[arg(
        long,
        value_enum,
        help = "Language preset that fills unset options with ecosystem defaults. `rust`: worktree backend, per-worker cache, warmup, CARGO_TARGET_DIR={build_cache}, probe `cargo test`. `go`: worktree backend, warmup, shared GOCACHE={build_cache}/go-build, GOTMPDIR={build_cache}, probe `go test ./...`. `node`: worktree backend, warmup, package-manager cache envs under {build_cache}, probe from lockfile detection (bun/pnpm/yarn/npm test). `python`: worktree backend, warmup, PYTHONPYCACHEPREFIX={build_cache}/pycache, PYTEST_ADDOPTS=--cache-clear, TMPDIR={build_cache}/tmp, probe `pytest`. `csharp`: worktree backend, warmup, DOTNET_CLI_TELEMETRY_OPTOUT=1, NUGET_PACKAGES={build_cache}/nuget, probe `dotnet test`. Explicit flags and ooze.toml win."
    )]
    pub(crate) preset: Option<PresetArg>,

    #[arg(long)]
    pub(crate) cache_dir: Option<PathBuf>,

    #[arg(long)]
    pub(crate) runs_dir: Option<PathBuf>,

    #[arg(
        long,
        value_enum,
        help = "Report format: json, human, agent-tasks-json, agent-tasks-markdown, github-annotations, sarif"
    )]
    pub(crate) format: Option<report::ReportFormat>,

    #[arg(long, help = "Write report to a file instead of stdout.")]
    pub(crate) output: Option<PathBuf>,

    #[arg(
        long,
        value_enum,
        help = "Report verbosity baseline: compact, normal, or full. Defaults per format (human/agent-tasks/sarif=compact, json=normal)."
    )]
    pub(crate) report_detail: Option<report::ReportDetail>,

    #[arg(long, help = "Omit unified diffs from the report.")]
    pub(crate) no_diff: bool,

    #[arg(long, help = "Omit probe stdout from the report.")]
    pub(crate) no_stdout: bool,

    #[arg(long, help = "Omit probe stderr from the report.")]
    pub(crate) no_stderr: bool,

    #[arg(long, help = "Keep only survived mutants in the report outcomes.")]
    pub(crate) only_survivors: bool,

    #[arg(
        long,
        value_delimiter = ',',
        help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git."
    )]
    pub(crate) exclude: Vec<String>,

    #[arg(
        long,
        value_name = "BASE",
        help = "Only mutate files changed versus BASE (e.g. `main`): git diff BASE...HEAD plus uncommitted and untracked changes."
    )]
    pub(crate) changed_only: Option<String>,

    #[arg(long = "probe-env", value_parser = parse_key_val, help = "KEY=VALUE env var to set on probe (and warmup). {worker} in VALUE expands to the worker index. Repeatable.")]
    pub(crate) probe_env: Vec<(String, String)>,

    #[arg(long, value_delimiter = ',', value_parser = parse_operator, help = "Restrict to these operators (comma-separated).")]
    pub(crate) operators: Vec<core::OperatorName>,

    #[arg(long = "exclude-operators", value_delimiter = ',', value_parser = parse_operator, help = "Drop these operators (comma-separated).")]
    pub(crate) exclude_operators: Vec<core::OperatorName>,

    #[arg(
        long,
        help = "Disable static skip rules (test files, assertion/panic macros, generated files)."
    )]
    pub(crate) no_static_skips: bool,

    #[arg(
        long,
        help = "Lines of source context around each survived mutant (0 disables)."
    )]
    pub(crate) context_lines: Option<usize>,

    #[arg(
        long,
        help = "Run the probe once on unmodified code first; abort if it fails or times out."
    )]
    pub(crate) preflight: bool,

    #[arg(
        long,
        help = "Exit 0 even if survivors are found (timeouts/errors still surface)."
    )]
    pub(crate) no_fail_on_survivors: bool,

    #[arg(
        long,
        help = "Treat timeout/error outcomes as non-fatal for exit code purposes."
    )]
    pub(crate) allow_incomplete: bool,

    #[arg(
        long,
        help = "Suppress per-mutant progress output (same as --progress never)."
    )]
    pub(crate) quiet: bool,

    #[arg(long, value_enum, default_value_t = ProgressMode::Auto, help = "Per-mutant progress on stderr: auto (TTY and not CI), always, or never.")]
    pub(crate) progress: ProgressMode,

    #[arg(last = true)]
    pub(crate) probe: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_is_json_only_for_json() {
        assert!(OutputFormat::Json.is_json());
        assert!(!OutputFormat::Human.is_json());
    }

    #[test]
    fn workspace_backend_arg_parses_worktree() {
        let arg = <WorkspaceBackendArg as ValueEnum>::from_str("worktree", true)
            .expect("worktree is a valid backend value");
        assert!(matches!(arg, WorkspaceBackendArg::Worktree));
    }

    #[test]
    fn explicit_backends_resolve_directly() {
        let dir = std::path::Path::new(".");
        assert_eq!(
            WorkspaceBackendArg::Worktree.resolve(dir),
            workspace::WorkspaceBackend::Worktree
        );
        assert_eq!(
            WorkspaceBackendArg::Copy.resolve(dir),
            workspace::WorkspaceBackend::Copy
        );
    }

    #[test]
    fn auto_prefers_worktree_inside_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(tmp.path())
            .args(["init", "-q"])
            .status()
            .expect("running git init")
            .success();
        assert!(ok);
        assert_eq!(
            WorkspaceBackendArg::Auto.resolve(tmp.path()),
            workspace::WorkspaceBackend::Worktree
        );
    }

    #[test]
    fn auto_falls_back_to_copy_outside_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            WorkspaceBackendArg::Auto.resolve(tmp.path()),
            workspace::WorkspaceBackend::Copy
        );
    }
}
