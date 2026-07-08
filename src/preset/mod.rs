//! Language preset policy: what each `--preset` value means at runtime.
//! The CLI surface in `crate::cli` only parses the flag (`cli::PresetArg`);
//! everything a preset *does* — marker detection, default fills, Node
//! package-manager handling — lives here.

mod node;

use std::path::Path;

pub(crate) use node::PackageManager;

/// A language preset: fills runner options the user left unset with good
/// defaults for that ecosystem. Explicit CLI flags and `ooze.toml` values
/// always win over preset defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Preset {
    Rust,
    Go,
    Node,
    Python,
    CSharp,
}

impl Preset {
    /// The preset's CLI value, for `--preset <name>` suggestions and the
    /// "ooze: preset <name>: ..." expansion line.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Preset::Rust => "rust",
            Preset::Go => "go",
            Preset::Node => "node",
            Preset::Python => "python",
            Preset::CSharp => "csharp",
        }
    }

    /// The fixed-name project marker files, at least one of which must exist
    /// at the project path for this preset to apply. Python is the only preset
    /// with alternatives: any of the common packaging files marks a project.
    /// C# has no fixed-name marker; see `marker_extensions`.
    pub(crate) fn marker_files(self) -> &'static [&'static str] {
        match self {
            Preset::Rust => &["Cargo.toml"],
            Preset::Go => &["go.mod"],
            Preset::Node => &["package.json"],
            Preset::Python => &[
                "pyproject.toml",
                "setup.py",
                "setup.cfg",
                "requirements.txt",
            ],
            Preset::CSharp => &[],
        }
    }

    /// Marker file extensions for presets whose project files have no fixed
    /// name (C#: any `*.sln` or `*.csproj`). Checked non-recursively at the
    /// project path.
    pub(crate) fn marker_extensions(self) -> &'static [&'static str] {
        match self {
            Preset::CSharp => &["sln", "csproj"],
            _ => &[],
        }
    }

    /// Whether the project path holds at least one of this preset's markers:
    /// a fixed-name file from `marker_files`, or (non-recursively) a file with
    /// one of `marker_extensions`.
    pub(crate) fn markers_present(self, path: &Path) -> bool {
        if self.marker_files().iter().any(|m| path.join(m).is_file()) {
            return true;
        }
        let extensions = self.marker_extensions();
        if extensions.is_empty() {
            return false;
        }
        std::fs::read_dir(path).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|x| extensions.contains(&x))
            })
        })
    }

    /// Human phrasing of the marker requirement for the "preset requires ..."
    /// error, e.g. "a Cargo.toml" or "one of pyproject.toml, ..., or
    /// requirements.txt".
    pub(crate) fn marker_requirement(self) -> String {
        match self {
            Preset::CSharp => "a .sln or .csproj".to_string(),
            _ => match self.marker_files() {
                [single] => format!("a {single}"),
                many => {
                    let (last, rest) = many.split_last().expect("presets have markers");
                    format!("one of {}, or {last}", rest.join(", "))
                }
            },
        }
    }

    /// The probe each preset falls back to when neither `--` args nor
    /// `[probe].command` supply one. Node's depends on the lockfile at `path`.
    pub(crate) fn test_command(self, path: &Path) -> &'static [&'static str] {
        match self {
            Preset::Rust => &["cargo", "test"],
            Preset::Go => &["go", "test", "./..."],
            Preset::Node => PackageManager::detect(path).test_command(),
            Preset::Python => &["pytest"],
            Preset::CSharp => &["dotnet", "test"],
        }
    }

    /// Probe-env defaults this preset fills when the user has not set the same
    /// key explicitly. Values are templates evaluated by the runner.
    pub(crate) fn probe_env_fills(self, path: &Path) -> &'static [(&'static str, &'static str)] {
        match self {
            Preset::Rust => &[("CARGO_TARGET_DIR", "{build_cache}")],
            Preset::Go => &[
                ("GOCACHE", "{build_cache}/go-build"),
                ("GOTMPDIR", "{build_cache}"),
            ],
            Preset::Node => PackageManager::detect(path).cache_env_fills(),
            Preset::Python => &[
                ("PYTHONPYCACHEPREFIX", "{build_cache}/pycache"),
                ("PYTEST_ADDOPTS", "--cache-clear"),
                ("TMPDIR", "{build_cache}/tmp"),
            ],
            Preset::CSharp => &[
                ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                ("NUGET_PACKAGES", "{build_cache}/nuget"),
            ],
        }
    }

    /// Every default this preset fills when neither a CLI flag nor `ooze.toml`
    /// sets the option, in the same `option=value` form `app::resolve` prints
    /// on its "ooze: preset <name>: ..." line. `doctor` shows this list so the
    /// recommended command is not a black box; keep the strings in sync with
    /// the fills in `app::resolve::test_mutants`.
    ///
    /// Go keeps the default shared build cache (`per_worker_cache=false`):
    /// Go's build cache is concurrency-safe by design, so workers share one
    /// GOCACHE. GOTMPDIR points at the same shared dir — the `go` command
    /// creates a unique work dir per invocation inside it — which keeps temp
    /// writes out of the system /tmp.
    ///
    /// Node also shares one cache: package-manager caches (npm/pnpm/yarn/bun)
    /// are safe to share across workers, while the workspace itself stays
    /// isolated by the worktree backend. Its probe and cache envs depend on
    /// the lockfile found at `path`, hence the parameter (the other presets
    /// ignore it).
    ///
    /// Python shares one cache root too: PYTHONPYCACHEPREFIX keeps `.pyc`
    /// writes out of the workspace, PYTEST_ADDOPTS=--cache-clear stops
    /// pytest's own cache from carrying state across mutants, and TMPDIR
    /// keeps probe temp files out of the system /tmp.
    ///
    /// C# shares one cache as well: the `NuGet` global packages folder is
    /// concurrency-safe, so `NUGET_PACKAGES` points every worker at
    /// `{build_cache}/nuget` while build outputs stay inside each isolated
    /// workspace. `DOTNET_CLI_TELEMETRY_OPTOUT` keeps probe runs quiet and
    /// network-free.
    pub(crate) fn fills(self, path: &Path) -> &'static [&'static str] {
        match self {
            Preset::Rust => &[
                "probe=`cargo test`",
                "workspace_backend=worktree",
                "per_worker_cache=true",
                "warmup=true",
                "probe_env += CARGO_TARGET_DIR={build_cache}",
            ],
            Preset::Go => &[
                "probe=`go test ./...`",
                "workspace_backend=worktree",
                "warmup=true",
                "probe_env += GOCACHE={build_cache}/go-build",
                "probe_env += GOTMPDIR={build_cache}",
            ],
            Preset::Python => &[
                "probe=`pytest`",
                "workspace_backend=worktree",
                "warmup=true",
                "probe_env += PYTHONPYCACHEPREFIX={build_cache}/pycache",
                "probe_env += PYTEST_ADDOPTS=--cache-clear",
                "probe_env += TMPDIR={build_cache}/tmp",
            ],
            Preset::CSharp => &[
                "probe=`dotnet test`",
                "workspace_backend=worktree",
                "warmup=true",
                "probe_env += DOTNET_CLI_TELEMETRY_OPTOUT=1",
                "probe_env += NUGET_PACKAGES={build_cache}/nuget",
            ],
            Preset::Node => match PackageManager::detect(path) {
                PackageManager::Bun => &[
                    "probe=`bun test`",
                    "workspace_backend=worktree",
                    "warmup=true",
                    "probe_env += BUN_INSTALL_CACHE_DIR={build_cache}/bun",
                ],
                PackageManager::Pnpm => &[
                    "probe=`pnpm test`",
                    "workspace_backend=worktree",
                    "warmup=true",
                    "probe_env += npm_config_cache={build_cache}/npm",
                    "probe_env += PNPM_HOME={build_cache}/pnpm-home",
                ],
                PackageManager::Yarn => &[
                    "probe=`yarn test`",
                    "workspace_backend=worktree",
                    "warmup=true",
                    "probe_env += YARN_CACHE_FOLDER={build_cache}/yarn",
                ],
                PackageManager::Npm => &[
                    "probe=`npm test`",
                    "workspace_backend=worktree",
                    "warmup=true",
                    "probe_env += npm_config_cache={build_cache}/npm",
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lowercase_cli_values() {
        let all = [
            Preset::Rust,
            Preset::Go,
            Preset::Node,
            Preset::Python,
            Preset::CSharp,
        ];
        assert_eq!(
            all.map(Preset::name),
            ["rust", "go", "node", "python", "csharp"]
        );
    }

    #[test]
    fn marker_requirement_wording_is_stable() {
        assert_eq!(Preset::Rust.marker_requirement(), "a Cargo.toml");
        assert_eq!(Preset::Go.marker_requirement(), "a go.mod");
        assert_eq!(Preset::Node.marker_requirement(), "a package.json");
        assert_eq!(
            Preset::Python.marker_requirement(),
            "one of pyproject.toml, setup.py, setup.cfg, or requirements.txt"
        );
        assert_eq!(Preset::CSharp.marker_requirement(), "a .sln or .csproj");
    }

    #[test]
    fn node_fills_follow_detected_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            Preset::Node.fills(tmp.path()).contains(&"probe=`npm test`"),
            "no lockfile falls back to npm"
        );
        std::fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        assert!(
            Preset::Node
                .fills(tmp.path())
                .contains(&"probe=`yarn test`")
        );
        std::fs::write(tmp.path().join("bun.lock"), "").unwrap();
        assert!(Preset::Node.fills(tmp.path()).contains(&"probe=`bun test`"));
    }
}
