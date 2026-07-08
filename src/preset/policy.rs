//! Typed runtime policy: everything a preset recommends for the runner, in
//! one value. `app::resolve` resolves the policy once and applies each field
//! only where neither a CLI flag nor `ooze.toml` set the option, so the
//! precedence (CLI > config > preset > built-in) lives entirely in resolve
//! while the *content* of each preset lives here.

use std::path::Path;

use crate::cli::WorkspaceBackendArg;

use super::{PackageManager, Preset};

/// What one preset recommends at runtime. Every field is a *default*: it
/// only takes effect when the user left the matching option unset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresetRuntimePolicy {
    /// The probe to fall back to when neither `--` args nor `[probe].command`
    /// supply one.
    pub(crate) default_probe: Vec<String>,
    /// The workspace backend to fill when none is chosen; `None` leaves the
    /// built-in `auto` default alone.
    pub(crate) workspace_backend: Option<WorkspaceBackendArg>,
    /// The warmup default to fill; `None` expresses no preset opinion.
    pub(crate) warmup: Option<bool>,
    /// Whether workers share one build cache or each get their own.
    pub(crate) cache_policy: CachePolicy,
    /// Probe-env defaults to append for keys the user hasn't set. Values are
    /// templates evaluated by the runner.
    pub(crate) probe_env: Vec<ProbeEnvFill>,
}

/// Build-cache sharing recommendation.
///
/// Only Rust turns on per-worker caches: Cargo target dirs fight over locks
/// when shared. The other ecosystems share one cache root by design:
///
/// * Go's build cache is concurrency-safe; GOTMPDIR points at the same shared
///   dir — the `go` command creates a unique work dir per invocation inside
///   it — which keeps temp writes out of the system /tmp.
/// * Node package-manager caches (npm/pnpm/yarn/bun) are safe to share across
///   workers, while the workspace itself stays isolated by the worktree
///   backend.
/// * Python: PYTHONPYCACHEPREFIX keeps `.pyc` writes out of the workspace,
///   PYTEST_ADDOPTS=--cache-clear stops pytest's own cache from carrying
///   state across mutants, and TMPDIR keeps probe temp files out of the
///   system /tmp.
/// * C#: the `NuGet` global packages folder is concurrency-safe, so
///   `NUGET_PACKAGES` points every worker at `{build_cache}/nuget` while
///   build outputs stay inside each isolated workspace.
///   `DOTNET_CLI_TELEMETRY_OPTOUT` keeps probe runs quiet and network-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachePolicy {
    /// One build cache shared by all workers (the built-in default, so this
    /// fills nothing during resolution).
    Shared,
    /// Each worker gets its own build cache (`per_worker_cache=true`).
    PerWorker,
}

/// One probe-env default: set `key` to the template `value` unless the user
/// already set `key` themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeEnvFill {
    pub(crate) key: &'static str,
    pub(crate) value: &'static str,
}

impl Preset {
    /// The full runtime policy for this preset. Node's probe and cache envs
    /// depend on the lockfile found at `path`, hence the parameter (the other
    /// presets ignore it).
    pub(crate) fn runtime_policy(self, path: &Path) -> PresetRuntimePolicy {
        let (probe, cache_policy, probe_env): (
            &[&str],
            CachePolicy,
            &[(&'static str, &'static str)],
        ) = match self {
            Preset::Rust => (
                &["cargo", "test"],
                CachePolicy::PerWorker,
                &[("CARGO_TARGET_DIR", "{build_cache}")],
            ),
            Preset::Go => (
                &["go", "test", "./..."],
                CachePolicy::Shared,
                &[
                    ("GOCACHE", "{build_cache}/go-build"),
                    ("GOTMPDIR", "{build_cache}"),
                ],
            ),
            Preset::Node => {
                let pm = PackageManager::detect(path);
                (pm.test_command(), CachePolicy::Shared, pm.cache_env_fills())
            }
            Preset::Python => (
                &["pytest"],
                CachePolicy::Shared,
                &[
                    ("PYTHONPYCACHEPREFIX", "{build_cache}/pycache"),
                    ("PYTEST_ADDOPTS", "--cache-clear"),
                    ("TMPDIR", "{build_cache}/tmp"),
                ],
            ),
            Preset::CSharp => (
                &["dotnet", "test"],
                CachePolicy::Shared,
                &[
                    ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                    ("NUGET_PACKAGES", "{build_cache}/nuget"),
                ],
            ),
        };
        PresetRuntimePolicy {
            default_probe: probe.iter().map(ToString::to_string).collect(),
            workspace_backend: Some(WorkspaceBackendArg::Worktree),
            warmup: Some(true),
            cache_policy,
            probe_env: probe_env
                .iter()
                .map(|&(key, value)| ProbeEnvFill { key, value })
                .collect(),
        }
    }
}

impl PresetRuntimePolicy {
    /// Every default this policy would fill, in the same `option=value` form
    /// `app::resolve` prints on its "ooze: preset <name>: ..." line. `doctor`
    /// shows this list so the recommended command is not a black box; resolve
    /// builds the same strings per applied fill, so the two cannot drift.
    pub(crate) fn fill_descriptions(&self) -> Vec<String> {
        let mut fills = vec![format!("probe=`{}`", self.default_probe.join(" "))];
        if let Some(backend) = self.workspace_backend {
            fills.push(format!("workspace_backend={}", backend.cli_name()));
        }
        if self.cache_policy == CachePolicy::PerWorker {
            fills.push("per_worker_cache=true".to_string());
        }
        if let Some(warmup) = self.warmup {
            fills.push(format!("warmup={warmup}"));
        }
        fills.extend(
            self.probe_env
                .iter()
                .map(|f| format!("probe_env += {}={}", f.key, f.value)),
        );
        fills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Preset; 5] = [
        Preset::Rust,
        Preset::Go,
        Preset::Node,
        Preset::Python,
        Preset::CSharp,
    ];

    #[test]
    fn only_rust_uses_per_worker_cache() {
        for p in ALL {
            let policy = p.runtime_policy(Path::new("."));
            let expected = if p == Preset::Rust {
                CachePolicy::PerWorker
            } else {
                CachePolicy::Shared
            };
            assert_eq!(policy.cache_policy, expected, "preset {}", p.name());
        }
    }

    #[test]
    fn default_probes_are_stable() {
        let probe = |p: Preset| p.runtime_policy(Path::new(".")).default_probe;
        assert_eq!(probe(Preset::Rust), ["cargo", "test"]);
        assert_eq!(probe(Preset::Go), ["go", "test", "./..."]);
        assert_eq!(probe(Preset::Python), ["pytest"]);
        assert_eq!(probe(Preset::CSharp), ["dotnet", "test"]);
        assert_eq!(probe(Preset::Node), ["npm", "test"], "no lockfile -> npm");
    }

    #[test]
    fn node_policy_follows_detected_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        let policy = Preset::Node.runtime_policy(tmp.path());
        assert_eq!(policy.default_probe, ["yarn", "test"]);
        assert_eq!(
            policy.probe_env,
            [ProbeEnvFill {
                key: "YARN_CACHE_FOLDER",
                value: "{build_cache}/yarn",
            }]
        );
        std::fs::write(tmp.path().join("bun.lock"), "").unwrap();
        assert_eq!(
            Preset::Node.runtime_policy(tmp.path()).default_probe,
            ["bun", "test"]
        );
    }

    #[test]
    fn every_preset_recommends_worktree_and_warmup() {
        for p in ALL {
            let policy = p.runtime_policy(Path::new("."));
            assert_eq!(
                policy.workspace_backend,
                Some(WorkspaceBackendArg::Worktree)
            );
            assert_eq!(policy.warmup, Some(true));
        }
    }

    #[test]
    fn probe_env_fills_are_stable() {
        let env = |p: Preset| -> Vec<(&str, &str)> {
            p.runtime_policy(Path::new("."))
                .probe_env
                .iter()
                .map(|f| (f.key, f.value))
                .collect()
        };
        assert_eq!(env(Preset::Rust), [("CARGO_TARGET_DIR", "{build_cache}")]);
        assert_eq!(
            env(Preset::Go),
            [
                ("GOCACHE", "{build_cache}/go-build"),
                ("GOTMPDIR", "{build_cache}"),
            ]
        );
        assert_eq!(
            env(Preset::Python),
            [
                ("PYTHONPYCACHEPREFIX", "{build_cache}/pycache"),
                ("PYTEST_ADDOPTS", "--cache-clear"),
                ("TMPDIR", "{build_cache}/tmp"),
            ]
        );
        assert_eq!(
            env(Preset::CSharp),
            [
                ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                ("NUGET_PACKAGES", "{build_cache}/nuget"),
            ]
        );
    }

    #[test]
    fn rust_fill_descriptions_are_stable() {
        assert_eq!(
            Preset::Rust
                .runtime_policy(Path::new("."))
                .fill_descriptions(),
            [
                "probe=`cargo test`",
                "workspace_backend=worktree",
                "per_worker_cache=true",
                "warmup=true",
                "probe_env += CARGO_TARGET_DIR={build_cache}",
            ]
        );
    }
}
