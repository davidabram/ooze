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
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MutationConfigToml {
    pub strategy: Option<String>,
    pub operators: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_operators: Vec<String>,
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
    pub shared_target: Option<bool>,
    pub warmup: Option<bool>,
    pub cache_dir: Option<PathBuf>,
    pub runs_dir: Option<PathBuf>,
    pub cargo_target_dir: Option<PathBuf>,
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
}
