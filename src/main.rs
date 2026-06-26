mod core;
mod source_path;
mod lang;
mod crap;
mod mutate;
mod runner;
mod skip;
mod scheduler;
mod report;
mod config;
mod doctor;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProgressMode {
    Auto,
    Always,
    Never,
}

impl ProgressMode {
    fn resolve(self) -> bool {
        match self {
            ProgressMode::Always => true,
            ProgressMode::Never => false,
            ProgressMode::Auto => {
                let ci = std::env::var_os("CI").is_some();
                !ci && std::io::stderr().is_terminal()
            }
        }
    }
}

pub(crate) const DEFAULT_EXCLUDES: &[&str] = &[
    "target/**",
    ".ooze/**",
    ".git/**",
    "node_modules/**",
    "vendor/**",
    "__pycache__/**",
    ".gradle/**",
];

const COVERAGE_HELP: &str = "Coverage report as `format:path` or a bare path to auto-detect. \
Formats: lcov, cobertura, jacoco, go-cover. Repeatable; multiple reports are merged. \
E.g. --coverage cobertura:coverage.xml";

/// Coverage resolved from the CLI, ready for scoring plus a count of how many
/// reports were merged (for diagnostics).
struct ResolvedCoverage {
    map: std::collections::HashMap<PathBuf, core::FileCoverage>,
    reports: usize,
}

/// Resolve coverage from the (repeatable) `--coverage` specs, falling back to
/// the deprecated `--lcov` flag. Returns `None` when neither was supplied.
fn resolve_coverage(
    coverage: &[String],
    lcov: Option<&std::path::Path>,
) -> anyhow::Result<Option<ResolvedCoverage>> {
    use crap::coverage::{CoverageFormat, CoverageInput};

    // `--coverage` specs take precedence; the deprecated `--lcov` flag is just an
    // implicit lcov-format input. Each spec is parsed to a typed input once here.
    let inputs: Vec<CoverageInput> = if !coverage.is_empty() {
        coverage
            .iter()
            .map(|spec| CoverageInput::parse(spec))
            .collect::<anyhow::Result<_>>()?
    } else if let Some(path) = lcov {
        vec![CoverageInput {
            format: CoverageFormat::Lcov,
            path: path.to_path_buf(),
        }]
    } else {
        return Ok(None);
    };

    Ok(Some(ResolvedCoverage {
        reports: inputs.len(),
        map: crap::coverage::load_inputs(&inputs)?,
    }))
}

/// Score `functions` against resolved coverage when present (printing match
/// diagnostics to stderr), or without coverage otherwise.
fn score_with_optional_coverage(
    functions: Vec<core::FunctionSpan>,
    coverage: Option<ResolvedCoverage>,
) -> Vec<core::CrapEntry> {
    match coverage {
        Some(ResolvedCoverage { map, reports }) => {
            report_coverage_match(reports, &functions, &map);
            crap::score_with_coverage(functions, &map)
        }
        None => crap::score_without_coverage(functions),
    }
}

/// Print how well the coverage reports line up with the scanned source tree.
fn report_coverage_match(
    reports: usize,
    functions: &[core::FunctionSpan],
    map: &std::collections::HashMap<PathBuf, core::FileCoverage>,
) {
    let mut scanned: Vec<PathBuf> = functions.iter().map(|f| f.file.clone()).collect();
    scanned.sort();
    scanned.dedup();

    let m = crap::match_report(&scanned, map);
    eprintln!("ooze: coverage reports parsed: {reports}");
    eprintln!("ooze: coverage source files:  {}", m.coverage_source_files);
    eprintln!("ooze: matched source files:   {}", m.matched_source_files);
    eprintln!("ooze: unmatched coverage files: {}", m.unmatched_coverage_files);
    eprintln!("ooze: unmatched scanned files:  {}", m.unmatched_source_files);
    if m.coverage_source_files > 0 && m.matched_source_files == 0 {
        eprintln!(
            "ooze: warning: no scanned files matched any coverage entry — check path roots (Docker/CI/monorepo prefixes)"
        );
    }
}

fn read_gitignore_patterns(root: &std::path::Path) -> Vec<String> {
    let path = root.join(".gitignore");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches('/').to_string())
        .collect()
}

fn parse_operator(s: &str) -> Result<core::OperatorName, String> {
    core::OperatorName::parse(s).ok_or_else(|| {
        let names: Vec<&str> = core::OperatorName::ALL.iter().copied().map(core::OperatorName::as_str).collect();
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
    let mut out: Vec<String> = DEFAULT_EXCLUDES.iter().map(std::string::ToString::to_string).collect();
    out.extend(read_gitignore_patterns(root));
    out.extend(user.iter().cloned());
    out
}

// Collects the set of files that differ from `base`, used by `--changed-only`.
// Returns canonical absolute paths so they can be matched against candidate
// file paths regardless of how `--path` was spelled. The union covers three
// sources: commits on this branch since the merge-base with `base`, working-tree
// modifications (staged and unstaged), and untracked-but-not-ignored files.
fn git_changed_files(
    base: &str,
    root: &std::path::Path,
) -> anyhow::Result<std::collections::HashSet<source_path::SourcePath>> {
    use anyhow::Context;

    let toplevel_out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running `git rev-parse --show-toplevel`")?;
    if !toplevel_out.status.success() {
        anyhow::bail!(
            "--changed-only: `git rev-parse` failed (is {} inside a git repo?): {}",
            root.display(),
            String::from_utf8_lossy(&toplevel_out.stderr).trim()
        );
    }
    let toplevel = PathBuf::from(String::from_utf8_lossy(&toplevel_out.stdout).trim());

    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    collect_git_paths(root, &["diff", "--name-only", &format!("{base}...HEAD")], &mut names)?;
    collect_git_paths(root, &["diff", "--name-only", "HEAD"], &mut names)?;
    collect_git_paths(root, &["ls-files", "--others", "--exclude-standard"], &mut names)?;

    // Resolve to source identities; drop entries that no longer exist (e.g.
    // deletions) since they carry no mutation candidates anyway.
    let mut out = std::collections::HashSet::new();
    for name in names {
        if let Some(id) = source_path::SourcePath::under(&toplevel, std::path::Path::new(&name)) {
            out.insert(id);
        }
    }
    Ok(out)
}

fn collect_git_paths(
    root: &std::path::Path,
    args: &[&str],
    out: &mut std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "--changed-only: `git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if !line.is_empty() {
            out.insert(line.to_string());
        }
    }
    Ok(())
}

// Keeps only candidates whose source file is among `changed`. Candidate files
// that fail to canonicalize (already gone) are dropped.
fn filter_candidates_to_changed(
    candidates: Vec<core::MutationCandidate>,
    changed: &std::collections::HashSet<source_path::SourcePath>,
) -> Vec<core::MutationCandidate> {
    candidates
        .into_iter()
        .filter(|c| {
            source_path::SourcePath::canonical(&c.file).is_some_and(|id| changed.contains(&id))
        })
        .collect()
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
        (0..jobs).map(|i| cache_dir.join(format!("build-cache-job-{i}"))).collect()
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
    probe_env: &[runner::ProbeEnvTemplate],
    num_workers: usize,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for t in probe_env {
        if !t.references_worker() {
            continue;
        }
        for i in 0..num_workers {
            let resolved = t.eval(runner::ProbeEnvCtx {
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
    if dirs.is_empty() {
        None
    } else {
        Some(dirs)
    }
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

/// Output shape for the simple introspection commands (scan, crap, mutants,
/// operators, plan-mutants, doctor): either machine JSON or a human rendering.
/// The richer `test-mutants` report uses `report::ReportFormat` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lower")]
enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    fn is_json(self) -> bool {
        matches!(self, OutputFormat::Json)
    }
}

fn parse_report_format_str(s: &str) -> anyhow::Result<report::ReportFormat> {
    <report::ReportFormat as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid report format {s:?}: {e}"))
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

fn parse_report_detail_str(s: &str) -> anyhow::Result<report::ReportDetail> {
    <report::ReportDetail as ValueEnum>::from_str(s, true)
        .map_err(|e| anyhow::anyhow!("invalid report detail {s:?}: {e}"))
}

#[derive(Subcommand)]
enum Commands {
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
        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,
    },
    #[command(about = "List available mutation operators and their metadata")]
    Operators {
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(about = "List supported languages and how far their support goes (scan-only vs mutation)")]
    Languages {
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,
    },
    #[command(about = "Plan a mutation run without executing probes: shows selection, scores, and applied excludes")]
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

        #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
        exclude: Vec<String>,

        #[arg(long, value_name = "BASE", help = "Only mutate files changed versus BASE (e.g. `main`): git diff BASE...HEAD plus uncommitted and untracked changes.")]
        changed_only: Option<String>,

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
    TestMutants(Box<TestMutantsArgs>),
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
    #[command(about = "Diagnose repo, config, and runtime preconditions for ooze")]
    Doctor {
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value = "human", help = "Output format: human or json")]
        format: OutputFormat,
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
struct TestMutantsArgs {
    #[arg(long, help = "Path to ooze.toml config (default: ./ooze.toml if present).")]
    config: Option<PathBuf>,

    #[arg(long, default_value = ".")]
    path: PathBuf,

    #[arg(long, help = "DEPRECATED: use --coverage. Path to an LCOV tracefile.")]
    lcov: Option<PathBuf>,

    #[arg(long, value_name = "SPEC", help = COVERAGE_HELP)]
    coverage: Vec<String>,

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

    #[arg(long, value_enum, help = "Report format: json, human, agent-tasks-json, agent-tasks-markdown, github-annotations, sarif")]
    format: Option<report::ReportFormat>,

    #[arg(long, help = "Write report to a file instead of stdout.")]
    output: Option<PathBuf>,

    #[arg(long, value_enum, help = "Report verbosity baseline: compact, normal, or full. Defaults per format (human/agent-tasks/sarif=compact, json=normal).")]
    report_detail: Option<report::ReportDetail>,

    #[arg(long, help = "Omit unified diffs from the report.")]
    no_diff: bool,

    #[arg(long, help = "Omit probe stdout from the report.")]
    no_stdout: bool,

    #[arg(long, help = "Omit probe stderr from the report.")]
    no_stderr: bool,

    #[arg(long, help = "Keep only survived mutants in the report outcomes.")]
    only_survivors: bool,

    #[arg(long, value_delimiter = ',', help = "Additional exclude globs (comma-separated). Defaults always exclude target, .ooze, .git.")]
    exclude: Vec<String>,

    #[arg(long, value_name = "BASE", help = "Only mutate files changed versus BASE (e.g. `main`): git diff BASE...HEAD plus uncommitted and untracked changes.")]
    changed_only: Option<String>,

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

    #[arg(long, help = "Suppress per-mutant progress output (same as --progress never).")]
    quiet: bool,

    #[arg(long, value_enum, default_value_t = ProgressMode::Auto, help = "Per-mutant progress on stderr: auto (TTY and not CI), always, or never.")]
    progress: ProgressMode,

    #[arg(last = true)]
    probe: Vec<String>,
}

#[derive(serde::Serialize)]
struct PlannedCandidate {
    #[serde(flatten)]
    candidate: core::MutationCandidate,
    #[serde(flatten)]
    selection: scheduler::SelectionExplanation,
}

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { path, format } => {
            let spans = lang::scan_directory(std::path::Path::new(&path))?;
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&spans)?);
            }
        }
        Commands::Mutants { path, format, exclude } => {
            let excludes = resolve_excludes(&path, &exclude);
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
                    OperatorEntry { info: op.info(), languages }
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
                        info.name, info.category, info.default_enabled, info.description, langs, info.test_hint
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
            format,
            exclude,
            changed_only,
            operators,
            exclude_operators,
            no_static_skips,
            show_skipped,
        } => {
            let excludes = resolve_excludes(&path, &exclude);
            let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);
            let registry = lang::CompiledRegistry::compile(lang::supported_languages(), &filter)?;
            let functions = lang::scan_directory_with_registry(&registry, &path, &excludes)?;
            let candidates = mutate::discover_mutants(&functions, &registry)?;
            let candidates = if let Some(base) = changed_only.as_deref() {
                let changed = git_changed_files(base, &path)?;
                filter_candidates_to_changed(candidates, &changed)
            } else {
                candidates
            };
            let total_candidates = candidates.len();
            let (candidates, skipped_candidates) = if no_static_skips {
                (candidates, Vec::new())
            } else {
                skip::partition(candidates)
            };
            let skipped_count = skipped_candidates.len();

            let coverage = resolve_coverage(&coverage, lcov.as_deref())?;
            let crap_entries = score_with_optional_coverage(functions, coverage);

            let mut ordered = scheduler::order(strategy, candidates, &crap_entries);
            if let Some(limit) = limit {
                ordered.truncate(limit);
            }

            let planned: Vec<PlannedCandidate> = ordered
                .into_iter()
                .map(|c| {
                    let selection = scheduler::explain(strategy, &c, &crap_entries);
                    PlannedCandidate { candidate: c, selection }
                })
                .collect();

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

            let workspace = runner::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;

            println!("{}", applied.diff);
        }
        Commands::TestMutants(args) => {
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
            } = *args;
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

            let probe_env: Vec<runner::ProbeEnvTemplate> =
                resolve_probe_env(probe_env, &cfg.probe.env)?
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

            let excludes = resolve_excludes(&path, &exclude);
            let filter = mutate::OperatorFilter::from_cli(&operators, &exclude_operators);
            let registry = lang::CompiledRegistry::compile(lang::supported_languages(), &filter)?;
            let functions = lang::scan_directory_with_registry(&registry, &path, &excludes)?;
            let candidates = mutate::discover_mutants(&functions, &registry)?;
            let candidates = if no_static_skips {
                candidates
            } else {
                let (kept, _) = skip::partition(candidates);
                kept
            };

            let changed_only = changed_only.or(cfg.scope.changed_only.clone());
            let candidates = if let Some(base) = changed_only.as_deref() {
                let changed = git_changed_files(base, &path)?;
                let before = candidates.len();
                let kept = filter_candidates_to_changed(candidates, &changed);
                eprintln!(
                    "ooze: --changed-only {base}: {} of {before} candidates in changed files",
                    kept.len()
                );
                kept
            } else {
                candidates
            };

            let coverage = resolve_coverage(&coverage, lcov.as_deref())?;
            let crap_entries = score_with_optional_coverage(functions, coverage);

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
                    let dirs = per_worker_cache_dirs(jobs, &cache_dir);
                    for d in &dirs {
                        std::fs::create_dir_all(d).with_context(|| {
                            format!("creating worker build cache dir {}", d.display())
                        })?;
                    }
                    (None, dirs)
                } else {
                    let dir = build_cache_dir
                        .unwrap_or_else(|| runner::default_build_cache_dir(&cache_dir));
                    std::fs::create_dir_all(&dir).with_context(|| {
                        format!("creating build cache dir {}", dir.display())
                    })?;
                    (Some(dir), Vec::new())
                };

            let num_workers = num_workers(jobs, &worker_build_cache_dirs);
            for p in worker_probe_env_dirs(&probe_env, num_workers) {
                std::fs::create_dir_all(&p).with_context(|| {
                    format!("creating probe-env directory {}", p.display())
                })?;
            }

            if preflight {
                let preflight_build_cache = target_dir
                    .as_deref()
                    .or_else(|| worker_build_cache_dirs.first().map(std::path::PathBuf::as_path));
                let preflight_envs = runner::template::eval_all(
                    &probe_env,
                    runner::ProbeEnvCtx {
                        worker: 0,
                        build_cache: preflight_build_cache,
                    },
                );
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
                    if matches!(format, report::ReportFormat::Human) {
                        eprintln!("Preflight failed.\n");
                        eprintln!("{msg}\n");
                        eprintln!("Command: {}", probe.join(" "));
                        if let Some(code) = payload.exit_code {
                            eprintln!("Exit code: {code}");
                        }
                    } else {
                        println!("{}", serde_json::to_string_pretty(&payload)?);
                    }
                    std::process::exit(report::OozeExitCode::PreflightFailed.code());
                }
            }

            if warmup {
                match warmup_target(&worker_build_cache_dirs, target_dir.as_deref()) {
                    WarmupTarget::Workers => {
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
                    }
                    WarmupTarget::Shared(dir) => {
                        eprintln!("warming up shared build cache dir...");
                        let extra = runner::template::eval_all(
                            &probe_env,
                            runner::ProbeEnvCtx {
                                worker: 0,
                                build_cache: Some(dir),
                            },
                        );
                        let status = runner::warmup(&repo_root, &probe, Some(dir), &extra)?;
                        warmup_status_to_result(status)?;
                    }
                    WarmupTarget::Nothing => {}
                }
            }

            let progress_enabled = progress_enabled(quiet, progress.resolve());
            let progress_cb: Option<fn(runner::ProgressEvent<'_>)> = if progress_enabled {
                Some(|ev: runner::ProgressEvent<'_>| {
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

            let cfg = runner::BatchConfig {
                backend: workspace_backend.resolve(),
                timeout,
                build_cache_dir: target_dir.as_deref(),
                worker_build_cache_dirs: worker_build_cache_arg(&worker_build_cache_dirs),
                probe_env_templates: &probe_env,
                runs_dir: &runs_dir,
                progress: progress_cb,
            };

            let raw_report = runner::run_mutants_parallel(
                &repo_root,
                candidates,
                &probe,
                jobs,
                &cfg,
            )?;

            let mut enriched = report::enrich(raw_report, &crap_entries, &repo_root, context_lines);
            report::apply_options(&mut enriched, report_opts);

            let text = format.render(&enriched)?;
            match output.as_deref() {
                Some(path) => std::fs::write(path, &text)
                    .with_context(|| format!("writing report to {}", path.display()))?,
                None => print!("{text}"),
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
            warmup_status_to_result(status)?;
        }
        Commands::TestMutant { path, id, probe } => {
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

            let workspace = runner::CowWorkspace::create_from_repo(&repo_root)?;
            let applied = workspace.apply_mutation(&repo_root, &candidate)?;
            let outcome = workspace.run_probe(applied, &probe, None)?;

            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Commands::Doctor { path, format } => {
            let report = doctor::run(&path);
            if format.is_json() {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::print_human(&report);
            }
            if report.has_failures() {
                std::process::exit(1);
            }
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
            coverage,
            format,
        } => {
            let functions = lang::scan_directory(std::path::Path::new(&path))?;
            let coverage = resolve_coverage(&coverage, lcov.as_deref())?;
            let entries = score_with_optional_coverage(functions, coverage);
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
        let out = resolve_operators(
            vec![],
            Some(&vec!["swap_boolean".to_string()]),
            None,
        )
        .unwrap();
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
        let out = resolve_exclude_operators(
            vec![],
            &["swap_boolean".to_string()],
            &[],
        )
        .unwrap();
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

    fn templates(pairs: &[(&str, &str)]) -> Vec<runner::ProbeEnvTemplate> {
        pairs
            .iter()
            .map(|(k, v)| runner::ProbeEnvTemplate::parse((*k).to_string(), v))
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
        assert_eq!(warmup_target(&[], Some(shared)), WarmupTarget::Shared(shared));
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

    // --- OutputFormat -----------------------------------------------------

    #[test]
    fn output_format_is_json_only_for_json() {
        assert!(OutputFormat::Json.is_json());
        assert!(!OutputFormat::Human.is_json());
    }
}
