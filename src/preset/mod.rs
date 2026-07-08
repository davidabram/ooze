//! Language preset policy: what each `--preset` value means at runtime.
//! The CLI surface in `crate::cli` only parses the flag (`cli::PresetArg`);
//! everything a preset *does* — marker detection, default fills, Node
//! package-manager handling — lives here.

mod node;
mod policy;

use std::path::Path;

pub(crate) use node::PackageManager;
pub(crate) use policy::{CachePolicy, ProbeEnvFill};

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
}
