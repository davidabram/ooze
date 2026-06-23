use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct OozeConfig {
    #[serde(default)]
    pub scope: ScopeConfigToml,
    #[serde(default)]
    pub mutation: MutationConfigToml,
    #[serde(default)]
    pub runner: RunnerConfigToml,
    #[serde(default)]
    pub probe: ProbeConfigToml,
    #[serde(default)]
    pub report: ReportConfigToml,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScopeConfigToml {
    #[serde(default)]
    pub exclude: Vec<String>,
    pub changed_only: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MutationConfigToml {
    pub strategy: Option<String>,
    pub operators: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_operators: Vec<String>,
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_categories: Vec<String>,
    pub static_skips: Option<bool>,
    pub context_lines: Option<usize>,
    pub lcov: Option<PathBuf>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RunnerConfigToml {
    pub workspace_backend: Option<String>,
    pub jobs: Option<usize>,
    pub timeout_seconds: Option<u64>,
    pub preflight: Option<bool>,
    pub per_worker_cache: Option<bool>,
    pub warmup: Option<bool>,
    pub cache_dir: Option<PathBuf>,
    pub runs_dir: Option<PathBuf>,
    pub build_cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProbeConfigToml {
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub env: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReportConfigToml {
    pub format: Option<String>,
    pub output: Option<PathBuf>,
    pub fail_on_survivors: Option<bool>,
    pub allow_incomplete: Option<bool>,
    /// Report verbosity baseline: "compact", "normal", or "full".
    pub detail: Option<String>,
    /// Include unified diffs (set false to drop them).
    pub diff: Option<bool>,
    /// Include probe stdout (set false to drop it).
    pub stdout: Option<bool>,
    /// Include probe stderr (set false to drop it).
    pub stderr: Option<bool>,
    /// Keep only survived mutants in the report outcomes.
    pub only_survivors: Option<bool>,
}
