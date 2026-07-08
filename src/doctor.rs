use std::path::{Path, PathBuf};

use crate::cli::Preset;
use crate::config::{self, OozeConfig};
use crate::core::{Language, OperatorName};
use crate::runner::{overlay, worktree};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Rust,
    Go,
    Node,
    Python,
    CSharp,
    /// More than one project type detected; `DoctorReport.detected` lists them.
    Mixed,
    Unknown,
}

impl ProjectType {
    pub fn human(self) -> &'static str {
        match self {
            ProjectType::Rust => "Rust/Cargo",
            ProjectType::Go => "Go",
            ProjectType::Node => "Node",
            ProjectType::Python => "Python",
            ProjectType::CSharp => "C#/.NET",
            ProjectType::Mixed => "mixed",
            ProjectType::Unknown => "unknown",
        }
    }

    /// The preset `recommend` suggests for this project type, if one exists.
    fn preset(self) -> Option<Preset> {
        match self {
            ProjectType::Rust => Some(Preset::Rust),
            ProjectType::Go => Some(Preset::Go),
            ProjectType::Node => Some(Preset::Node),
            ProjectType::Python => Some(Preset::Python),
            ProjectType::CSharp => Some(Preset::CSharp),
            ProjectType::Mixed | ProjectType::Unknown => None,
        }
    }

    /// Languages whose mutation operators apply to this project type. Node
    /// projects can hold both JavaScript and TypeScript sources.
    fn languages(self) -> &'static [Language] {
        match self {
            ProjectType::Rust => &[Language::Rust],
            ProjectType::Go => &[Language::Go],
            ProjectType::Node => &[Language::JavaScript, Language::TypeScript],
            ProjectType::Python => &[Language::Python],
            ProjectType::CSharp => &[Language::CSharp],
            ProjectType::Mixed | ProjectType::Unknown => &[],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GitStatus {
    pub available: bool,
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendStatus {
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendsStatus {
    pub worktree: BackendStatus,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStatus {
    pub sccache: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PresetFill {
    /// The default, in the same `option=value` form `test-mutants` prints
    /// when it expands the preset.
    pub fill: String,
    /// `Some` when `ooze.toml` already sets this option, so the preset will
    /// leave it alone; holds the winning config entry for display.
    pub overridden_by: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Recommendation {
    /// `None` when no preset applies; the human output then suggests passing
    /// a probe manually.
    pub command: Option<String>,
    /// What `--preset` in `command` would fill, each checked against the
    /// loaded `ooze.toml`. Empty when `command` is `None`. Fills overridden
    /// by an explicit flag in `command` itself (e.g. `--workspace-backend
    /// copy`) are omitted entirely.
    pub preset_fills: Vec<PresetFill>,
    /// For mixed projects only: one suggested command per detected type. No
    /// single recommendation is selected automatically, so `command` stays
    /// `None` and no fills are listed.
    pub mixed_commands: Vec<String>,
}

/// Operator support for one language, split by default enablement. Derived
/// from the same mutator registry mutation discovery reads, so this listing
/// cannot drift from the implementations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LanguageOperators {
    pub language: Language,
    pub enabled_by_default: Vec<&'static str>,
    pub disabled_by_default: Vec<&'static str>,
}

fn language_operators(language: Language) -> LanguageOperators {
    let mut enabled_by_default = Vec::new();
    let mut disabled_by_default = Vec::new();
    for m in crate::mutate::registry::implementations_for_language(language) {
        if m.default_enabled() {
            enabled_by_default.push(m.operator.as_str());
        } else {
            disabled_by_default.push(m.operator.as_str());
        }
    }
    LanguageOperators {
        language,
        enabled_by_default,
        disabled_by_default,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub path: PathBuf,
    pub project_type: ProjectType,
    /// Every project type whose marker file was found; more than one entry
    /// makes `project_type` `Mixed`, none makes it `Unknown`.
    pub detected: Vec<ProjectType>,
    pub git: GitStatus,
    pub backends: BackendsStatus,
    pub cache: CacheStatus,
    pub recommendation: Recommendation,
    /// `Some` only when `doctor --operators` is passed: one entry per language
    /// of each detected project type (empty when nothing was detected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operators: Option<Vec<LanguageOperators>>,
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
                Some(p) => ok("probe_command", format!("{} found at {}", bin, p.display())),
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

/// Every project type whose marker file exists at `path`, in a fixed order so
/// mixed-project output is deterministic.
fn detect_project_types(path: &Path) -> Vec<ProjectType> {
    let mut types = Vec::new();
    if path.join("Cargo.toml").is_file() {
        types.push(ProjectType::Rust);
    }
    if path.join("go.mod").is_file() {
        types.push(ProjectType::Go);
    }
    if path.join("package.json").is_file() {
        types.push(ProjectType::Node);
    }
    if Preset::Python
        .marker_files()
        .iter()
        .any(|m| path.join(m).is_file())
    {
        types.push(ProjectType::Python);
    }
    if Preset::CSharp.markers_present(path) {
        types.push(ProjectType::CSharp);
    }
    types
}

fn summarize_project_type(detected: &[ProjectType]) -> ProjectType {
    match detected {
        [] => ProjectType::Unknown,
        [single] => *single,
        _ => ProjectType::Mixed,
    }
}

fn detect_git(path: &Path) -> GitStatus {
    let root = worktree::git_toplevel(path);
    GitStatus {
        available: root.is_some(),
        root,
    }
}

fn worktree_backend_status(path: &Path, git: &GitStatus) -> BackendStatus {
    if which("git").is_none() {
        return BackendStatus {
            available: false,
            reason: Some("git is not installed".to_string()),
        };
    }
    if !git.available {
        return BackendStatus {
            available: false,
            reason: Some("not inside a Git repository".to_string()),
        };
    }
    let usable = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["worktree", "list"])
        .output()
        .is_ok_and(|o| o.status.success());
    if usable {
        BackendStatus {
            available: true,
            reason: None,
        }
    } else {
        BackendStatus {
            available: false,
            reason: Some("`git worktree list` failed in this repository".to_string()),
        }
    }
}

/// The `ooze.toml` entry that stops a preset from applying `fill`, if any.
/// Mirrors the suppression rules in `app::resolve::test_mutants`: a set
/// config key wins over the preset regardless of its value.
fn preset_fill_override(fill: &str, cfg: &OozeConfig) -> Option<String> {
    if fill.starts_with("probe=") {
        cfg.probe
            .command
            .as_ref()
            .map(|c| format!("[probe].command = `{}`", c.join(" ")))
    } else if fill.starts_with("workspace_backend=") {
        cfg.runner
            .workspace_backend
            .as_ref()
            .map(|b| format!("[runner].workspace_backend = \"{b}\""))
    } else if fill.starts_with("per_worker_cache=") {
        cfg.runner
            .per_worker_cache
            .map(|v| format!("[runner].per_worker_cache = {v}"))
    } else if fill.starts_with("warmup=") {
        cfg.runner.warmup.map(|v| format!("[runner].warmup = {v}"))
    } else if let Some(env_fill) = fill.strip_prefix("probe_env += ") {
        let key = env_fill.split('=').next()?;
        cfg.probe
            .env
            .iter()
            .find(|e| e.split('=').next() == Some(key))
            .map(|e| format!("[probe].env has `{e}`"))
    } else {
        None
    }
}

/// The recommended command and fill list for one preset, respecting the
/// worktree backend's availability.
fn preset_recommendation(
    preset: Preset,
    path: &Path,
    worktree: &BackendStatus,
    cfg: &OozeConfig,
) -> (String, Vec<PresetFill>) {
    let fills = || {
        preset.fills(path).iter().map(|f| PresetFill {
            fill: f.to_string(),
            overridden_by: preset_fill_override(f, cfg),
        })
    };
    if worktree.available {
        (
            format!("ooze test-mutants --preset {}", preset.name()),
            fills().collect(),
        )
    } else {
        // The explicit --workspace-backend flag wins over the preset, so
        // the worktree fill would never apply; drop it from the list.
        (
            format!(
                "ooze test-mutants --preset {} --workspace-backend copy",
                preset.name()
            ),
            fills()
                .filter(|f| !f.fill.starts_with("workspace_backend="))
                .collect(),
        )
    }
}

fn recommend(
    detected: &[ProjectType],
    path: &Path,
    worktree: &BackendStatus,
    cfg: &OozeConfig,
) -> Recommendation {
    let presets: Vec<Preset> = detected.iter().filter_map(|t| t.preset()).collect();
    match presets.as_slice() {
        [] => Recommendation {
            command: None,
            preset_fills: Vec::new(),
            mixed_commands: Vec::new(),
        },
        [single] => {
            let (command, preset_fills) = preset_recommendation(*single, path, worktree, cfg);
            Recommendation {
                command: Some(command),
                preset_fills,
                mixed_commands: Vec::new(),
            }
        }
        many => Recommendation {
            command: None,
            preset_fills: Vec::new(),
            mixed_commands: many
                .iter()
                .map(|p| preset_recommendation(*p, path, worktree, cfg).0)
                .collect(),
        },
    }
}

pub fn run(path: &Path, include_operators: bool) -> DoctorReport {
    let mut checks: Vec<CheckResult> = Vec::new();

    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => {
            if p.is_dir() {
                checks.push(ok("repo_root", format!("{} is a directory", p.display())));
                p
            } else {
                checks.push(fail(
                    "repo_root",
                    format!("{} is not a directory", p.display()),
                ));
                p
            }
        }
        Err(e) => {
            checks.push(fail(
                "repo_root",
                format!("cannot canonicalize {}: {e}", path.display()),
            ));
            path.to_path_buf()
        }
    };

    let detected = detect_project_types(&canonical);
    let project_type = summarize_project_type(&detected);
    let operators = include_operators.then(|| {
        detected
            .iter()
            .flat_map(|t| t.languages())
            .map(|l| language_operators(*l))
            .collect()
    });
    let git = detect_git(&canonical);
    let worktree_status = worktree_backend_status(&canonical, &git);
    let cache = CacheStatus {
        sccache: which("sccache").is_some(),
    };

    let cfg_path = canonical.join(config::DEFAULT_CONFIG_NAME);
    let (cfg, cfg_loaded_from) = if cfg_path.exists() {
        match config::load_config(Some(&cfg_path), &canonical) {
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

    // After config load so preset fills can be checked against ooze.toml
    // (a parse failure falls back to defaults, i.e. every fill shows active).
    let recommendation = recommend(&detected, &canonical, &worktree_status, &cfg);

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
        "worktree" => {
            if worktree::is_git_repo(&canonical) {
                checks.push(ok(
                    "workspace_backend",
                    "worktree requested and inside a Git repository",
                ));
            } else {
                checks.push(fail(
                    "workspace_backend",
                    "worktree requested but not inside a Git repository (run `git init` or use copy)",
                ));
            }
        }
        _ => {
            let resolved = if worktree::is_git_repo(&canonical) {
                "worktree"
            } else {
                "copy"
            };
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
            let resolved = if p.is_absolute() {
                p.clone()
            } else {
                canonical.join(p)
            };
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
        crate::app::DEFAULT_EXCLUDES.len(),
        cfg.scope.exclude.len()
    );
    if gitignore.is_file() {
        let lines = std::fs::read_to_string(&gitignore)
            .map_or(0, |s| s.lines().filter(|l| is_gitignore_pattern(l)).count());
        excludes_msg = format!("{excludes_msg}, gitignore={lines}");
        checks.push(ok("excludes", excludes_msg));
    } else {
        checks.push(warn(
            "excludes",
            format!("{excludes_msg}, no .gitignore at repo root"),
        ));
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
        project_type,
        detected,
        git,
        backends: BackendsStatus {
            worktree: worktree_status,
        },
        cache,
        recommendation,
        operators,
        config_path: cfg_loaded_from,
        checks,
        failed,
        warned,
    }
}

fn language_human(lang: Language) -> &'static str {
    match lang {
        Language::Rust => "Rust",
        Language::Go => "Go",
        Language::Python => "Python",
        Language::JavaScript => "JavaScript",
        Language::TypeScript => "TypeScript",
        Language::CSharp => "C#",
        other => other.as_str(),
    }
}

fn find_ops(ops: &[LanguageOperators], lang: Language) -> Option<&LanguageOperators> {
    ops.iter().find(|o| o.language == lang)
}

/// The operator sections to render for one project type: normally one per
/// language, but JavaScript and TypeScript collapse into a single combined
/// section when their operator support is identical (which it is today; the
/// comparison keeps the label honest if they ever diverge).
fn operator_sections(
    project_type: ProjectType,
    ops: &[LanguageOperators],
) -> Vec<(&'static str, &LanguageOperators)> {
    if project_type == ProjectType::Node
        && let (Some(js), Some(ts)) = (
            find_ops(ops, Language::JavaScript),
            find_ops(ops, Language::TypeScript),
        )
        && js.enabled_by_default == ts.enabled_by_default
        && js.disabled_by_default == ts.disabled_by_default
    {
        return vec![("JavaScript/TypeScript", js)];
    }
    project_type
        .languages()
        .iter()
        .filter_map(|l| find_ops(ops, *l).map(|o| (language_human(*l), o)))
        .collect()
}

fn print_operator_groups(o: &LanguageOperators, indent: usize) {
    let pad = " ".repeat(indent);
    if o.enabled_by_default.is_empty() && o.disabled_by_default.is_empty() {
        println!("{pad}no mutation operators registered for this language yet");
        return;
    }
    println!("{pad}enabled by default:");
    for op in &o.enabled_by_default {
        println!("{pad}  {op}");
    }
    if !o.disabled_by_default.is_empty() {
        println!("{pad}available but disabled by default:");
        for op in &o.disabled_by_default {
            println!("{pad}  {op}");
        }
    }
}

fn print_operators(report: &DoctorReport, ops: &[LanguageOperators]) {
    println!("Operators");
    if report.detected.is_empty() {
        println!("  No language detected.");
        println!(
            "  Try running from a supported project or specify a preset/test command manually."
        );
        return;
    }
    if report.project_type == ProjectType::Mixed {
        for (i, ty) in report.detected.iter().enumerate() {
            if i > 0 {
                println!();
            }
            println!("  {}", ty.human());
            let nested = ty.languages().len() > 1;
            for (label, o) in operator_sections(*ty, ops) {
                if nested {
                    println!("    {label}");
                    print_operator_groups(o, 6);
                } else {
                    print_operator_groups(o, 4);
                }
            }
        }
    } else {
        for (label, o) in operator_sections(report.project_type, ops) {
            println!("  language: {label}");
            print_operator_groups(o, 2);
        }
    }
    let disabled: std::collections::BTreeSet<&str> = ops
        .iter()
        .flat_map(|o| o.disabled_by_default.iter().copied())
        .collect();
    if !disabled.is_empty() {
        let list: Vec<&str> = disabled.into_iter().collect();
        println!();
        println!("  To include disabled operators:");
        println!("    ooze test-mutants --operators {} ...", list.join(","));
    }
}

pub fn print_human(report: &DoctorReport) {
    println!("ooze doctor");
    println!();
    println!("Project");
    println!("  path: {}", report.path.display());
    println!("  type: {}", report.project_type.human());
    if report.project_type == ProjectType::Mixed {
        let names: Vec<&str> = report.detected.iter().map(|t| t.human()).collect();
        println!("  detected: {}", names.join(", "));
    }
    if let Some(ops) = &report.operators {
        println!();
        print_operators(report, ops);
    }
    println!();
    println!("Git");
    if report.git.available {
        println!("  git repo: found");
        if let Some(root) = &report.git.root {
            println!("  root: {}", root.display());
        }
    } else {
        println!("  git repo: not found");
    }
    if report.backends.worktree.available {
        println!("  worktree backend: available");
    } else {
        println!("  worktree backend: unavailable");
        if let Some(reason) = &report.backends.worktree.reason {
            println!("  reason: {reason}");
        }
    }
    println!();
    println!("Cache");
    println!(
        "  sccache: {}",
        if report.cache.sccache {
            "found"
        } else {
            "not found"
        }
    );
    if report.detected.contains(&ProjectType::Rust) {
        println!("  recommended Rust cache: per-worker CARGO_TARGET_DIR={{build_cache}}");
        if report.cache.sccache {
            println!("  sccache is opt-in: add --probe-env RUSTC_WRAPPER=sccache");
        }
    }
    if report.detected.contains(&ProjectType::Go) {
        println!("  recommended Go cache: shared GOCACHE={{build_cache}}/go-build");
    }
    if report.detected.contains(&ProjectType::Node) {
        let pm = crate::cli::PackageManager::detect(&report.path);
        let envs: Vec<String> = pm
            .cache_env_fills()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        println!("  recommended Node cache: shared {}", envs.join(", "));
    }
    if report.detected.contains(&ProjectType::Python) {
        println!("  recommended Python cache: shared PYTHONPYCACHEPREFIX={{build_cache}}/pycache");
    }
    if report.detected.contains(&ProjectType::CSharp) {
        println!("  recommended C# cache: shared NUGET_PACKAGES={{build_cache}}/nuget");
    }
    println!();
    println!("Recommendation");
    if let Some(cmd) = &report.recommendation.command {
        println!("  {cmd}");
        if !report.recommendation.preset_fills.is_empty() {
            println!("  the preset fills options you leave unset (CLI flags and ooze.toml win):");
            for f in &report.recommendation.preset_fills {
                match &f.overridden_by {
                    None => println!("    {}", f.fill),
                    Some(winner) => {
                        println!("    {} (inactive: ooze.toml wins with {winner})", f.fill)
                    }
                }
            }
        }
    } else if !report.recommendation.mixed_commands.is_empty() {
        println!("  Multiple project types detected; no single preset is selected automatically.");
        println!("  Pick the one matching the code you want to mutate:");
        for cmd in &report.recommendation.mixed_commands {
            println!("    {cmd}");
        }
    } else {
        println!("  No preset recommendation available yet.");
        println!("  Try specifying a probe manually, for example:");
        println!("    ooze test-mutants -- <your test command>");
    }
    println!();
    println!("Checks");
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

    fn write_cargo_toml(dir: &Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
    }

    fn init_git_repo(dir: &Path) {
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .status()
                .expect("running git");
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
    }

    #[test]
    fn cargo_git_repo_detects_rust_git_and_recommends_preset() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_cargo_toml(dir.path());
        init_git_repo(dir.path());

        let report = run(dir.path(), false);
        assert_eq!(report.project_type, ProjectType::Rust);
        assert!(report.git.available, "git repo should be found");
        assert_eq!(
            report.git.root.as_deref(),
            Some(report.path.as_path()),
            "git root should be the repo itself"
        );
        assert!(report.backends.worktree.available);
        assert_eq!(report.backends.worktree.reason, None);
        assert_eq!(
            report.recommendation.command.as_deref(),
            Some("ooze test-mutants --preset rust")
        );
        let fills: Vec<&str> = report
            .recommendation
            .preset_fills
            .iter()
            .map(|f| f.fill.as_str())
            .collect();
        assert_eq!(fills, Preset::Rust.fills(dir.path()));
        assert!(
            report
                .recommendation
                .preset_fills
                .iter()
                .all(|f| f.overridden_by.is_none()),
            "no ooze.toml in the fixture, so every fill is active"
        );
    }

    #[test]
    fn non_git_cargo_dir_detects_rust_but_worktree_unavailable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_cargo_toml(dir.path());

        let report = run(dir.path(), false);
        assert_eq!(report.project_type, ProjectType::Rust);
        assert!(!report.git.available, "no git repo should be found");
        assert!(!report.backends.worktree.available);
        assert!(
            report
                .backends
                .worktree
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("Git repository")),
            "reason should explain the missing repo: {:?}",
            report.backends.worktree.reason
        );
        assert_eq!(
            report.recommendation.command.as_deref(),
            Some("ooze test-mutants --preset rust --workspace-backend copy")
        );
        assert!(
            !report
                .recommendation
                .preset_fills
                .iter()
                .any(|f| f.fill.starts_with("workspace_backend=")),
            "explicit --workspace-backend copy makes the worktree fill dead: {:?}",
            report.recommendation.preset_fills
        );
        assert!(
            report
                .recommendation
                .preset_fills
                .iter()
                .any(|f| f.fill == "probe=`cargo test`"),
            "remaining fills still listed: {:?}",
            report.recommendation.preset_fills
        );
    }

    #[test]
    fn ooze_toml_settings_mark_preset_fills_overridden() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_cargo_toml(dir.path());
        std::fs::write(
            dir.path().join(config::DEFAULT_CONFIG_NAME),
            "[runner]\nwarmup = false\nworkspace_backend = \"copy\"\n\n\
             [probe]\ncommand = [\"cargo\", \"nextest\", \"run\"]\n\
             env = [\"CARGO_TARGET_DIR=/tmp/t\"]\n",
        )
        .expect("write config");

        let report = run(dir.path(), false);
        let override_of = |prefix: &str| {
            report
                .recommendation
                .preset_fills
                .iter()
                .find(|f| f.fill.starts_with(prefix))
                .unwrap_or_else(|| panic!("fill {prefix:?} missing"))
                .overridden_by
                .clone()
        };
        assert_eq!(
            override_of("warmup=").as_deref(),
            Some("[runner].warmup = false")
        );
        assert_eq!(
            override_of("probe=").as_deref(),
            Some("[probe].command = `cargo nextest run`")
        );
        assert_eq!(
            override_of("probe_env").as_deref(),
            Some("[probe].env has `CARGO_TARGET_DIR=/tmp/t`")
        );
        // Note: no git repo in this fixture, so the recommended command pins
        // --workspace-backend copy and the worktree fill is omitted, even
        // though the config also sets a backend.
        assert!(override_of("per_worker_cache=").is_none());
    }

    #[test]
    fn csproj_dir_detects_csharp_and_recommends_preset() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Sample.Tests.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n",
        )
        .expect("write csproj");

        let report = run(dir.path(), false);
        assert_eq!(report.project_type, ProjectType::CSharp);
        assert!(
            report
                .recommendation
                .command
                .as_deref()
                .is_some_and(|c| c.starts_with("ooze test-mutants --preset csharp")),
            "expected a csharp preset recommendation: {:?}",
            report.recommendation.command
        );
        let fills: Vec<&str> = report
            .recommendation
            .preset_fills
            .iter()
            .map(|f| f.fill.as_str())
            .collect();
        assert!(fills.contains(&"probe=`dotnet test`"), "fills: {fills:?}");
        assert!(
            fills.contains(&"probe_env += NUGET_PACKAGES={build_cache}/nuget"),
            "fills: {fills:?}"
        );
    }

    #[test]
    fn mixed_detection_includes_csharp() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_cargo_toml(dir.path());
        std::fs::write(dir.path().join("Sample.sln"), "\n").expect("write sln");

        let report = run(dir.path(), false);
        assert_eq!(report.project_type, ProjectType::Mixed);
        assert_eq!(
            report.detected,
            vec![ProjectType::Rust, ProjectType::CSharp]
        );
        assert!(
            report
                .recommendation
                .mixed_commands
                .iter()
                .any(|c| c.starts_with("ooze test-mutants --preset csharp")),
            "mixed commands should include csharp: {:?}",
            report.recommendation.mixed_commands
        );
    }

    #[test]
    fn unknown_dir_reports_unknown_type_and_no_recommendation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = run(dir.path(), false);
        assert_eq!(report.project_type, ProjectType::Unknown);
        assert_eq!(report.recommendation.command, None);
        assert!(report.recommendation.preset_fills.is_empty());
    }

    #[test]
    fn report_is_built_regardless_of_sccache_presence() {
        // sccache is optional: whatever the machine has, the report must come
        // out whole with the flag simply reflecting PATH lookup.
        let dir = tempfile::tempdir().expect("tempdir");
        let report = run(dir.path(), false);
        assert_eq!(report.cache.sccache, which("sccache").is_some());
    }

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
        let c = probe_command_check(Some(&vec!["/no/such/binary/xyzzy".to_string()]));
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

        let report = run(dir.path(), false);
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
    fn language_operators_mirror_the_registry() {
        // The helper must agree with the registry (the data mutation discovery
        // reads) for every language a project type can map to, so the doctor
        // listing cannot drift from the implementations.
        for lang in [
            Language::Rust,
            Language::Go,
            Language::JavaScript,
            Language::TypeScript,
            Language::Python,
            Language::CSharp,
        ] {
            let ops = language_operators(lang);
            for m in crate::mutate::registry::implementations_for_language(lang) {
                let group = if m.default_enabled() {
                    &ops.enabled_by_default
                } else {
                    &ops.disabled_by_default
                };
                assert!(
                    group.contains(&m.operator.as_str()),
                    "{lang}: {} missing from its default-enablement group",
                    m.operator.as_str()
                );
            }
            let total = crate::mutate::registry::implementations_for_language(lang).count();
            assert_eq!(
                ops.enabled_by_default.len() + ops.disabled_by_default.len(),
                total,
                "{lang}: listing must cover every registered implementation"
            );
        }
    }

    #[test]
    fn rust_operators_list_integer_zero_one_as_disabled() {
        let ops = language_operators(Language::Rust);
        assert!(
            ops.disabled_by_default.contains(&"integer_zero_one"),
            "integer_zero_one should be disabled by default for Rust: {:?}",
            ops.disabled_by_default
        );
        assert!(
            ops.enabled_by_default.contains(&"swap_boolean"),
            "swap_boolean should be enabled by default for Rust: {:?}",
            ops.enabled_by_default
        );
    }

    #[test]
    fn csharp_operators_follow_the_global_default_convention() {
        let ops = language_operators(Language::CSharp);
        assert!(
            ops.enabled_by_default.contains(&"swap_boolean"),
            "swap_boolean should be enabled by default for C#: {:?}",
            ops.enabled_by_default
        );
        assert!(
            ops.disabled_by_default.contains(&"integer_zero_one"),
            "integer_zero_one should be disabled by default for C#: {:?}",
            ops.disabled_by_default
        );
    }

    #[test]
    fn node_sections_combine_js_and_ts_while_identical() {
        // JS and TS currently register identical operator support, so the
        // human output shows one combined section. If they diverge,
        // operator_sections must fall back to separate sections instead of
        // printing a wrong combined label.
        let ops = vec![
            language_operators(Language::JavaScript),
            language_operators(Language::TypeScript),
        ];
        let sections = operator_sections(ProjectType::Node, &ops);
        let labels: Vec<&str> = sections.iter().map(|(l, _)| *l).collect();
        if ops[0].enabled_by_default == ops[1].enabled_by_default
            && ops[0].disabled_by_default == ops[1].disabled_by_default
        {
            assert_eq!(labels, vec!["JavaScript/TypeScript"]);
        } else {
            assert_eq!(labels, vec!["JavaScript", "TypeScript"]);
        }
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

        let report = run(dir.path(), false);
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
