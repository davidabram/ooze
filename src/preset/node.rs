//! Node package-manager policy for the `node` preset.

use std::path::Path;

/// The Node package manager the `node` preset targets, picked by lockfile.
/// Everything the preset fills for Node — probe and cache envs — hangs off
/// this choice, so detection lives in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageManager {
    Bun,
    Pnpm,
    Yarn,
    Npm,
}

impl PackageManager {
    /// Deterministic lockfile detection at the project path. When several
    /// lockfiles coexist the priority is bun > pnpm > yarn > npm; a bare
    /// `package.json` with no lockfile falls back to npm.
    pub(crate) fn detect(path: &Path) -> PackageManager {
        if path.join("bun.lockb").is_file() || path.join("bun.lock").is_file() {
            PackageManager::Bun
        } else if path.join("pnpm-lock.yaml").is_file() {
            PackageManager::Pnpm
        } else if path.join("yarn.lock").is_file() {
            PackageManager::Yarn
        } else {
            PackageManager::Npm
        }
    }

    /// The default probe when the node preset has to supply one.
    pub(crate) fn test_command(self) -> &'static [&'static str] {
        match self {
            PackageManager::Bun => &["bun", "test"],
            PackageManager::Pnpm => &["pnpm", "test"],
            PackageManager::Yarn => &["yarn", "test"],
            PackageManager::Npm => &["npm", "test"],
        }
    }

    /// Probe-env defaults pointing this package manager's cache into the
    /// shared build-cache dir. pnpm also gets `npm_config_cache` because it
    /// shells out to npm for some operations.
    pub(crate) fn cache_env_fills(self) -> &'static [(&'static str, &'static str)] {
        match self {
            PackageManager::Bun => &[("BUN_INSTALL_CACHE_DIR", "{build_cache}/bun")],
            PackageManager::Pnpm => &[
                ("npm_config_cache", "{build_cache}/npm"),
                ("PNPM_HOME", "{build_cache}/pnpm-home"),
            ],
            PackageManager::Yarn => &[("YARN_CACHE_FOLDER", "{build_cache}/yarn")],
            PackageManager::Npm => &[("npm_config_cache", "{build_cache}/npm")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), "").unwrap();
    }

    #[test]
    fn detect_priority_is_bun_pnpm_yarn_npm() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(PackageManager::detect(dir), PackageManager::Npm);
        touch(dir, "yarn.lock");
        assert_eq!(PackageManager::detect(dir), PackageManager::Yarn);
        touch(dir, "pnpm-lock.yaml");
        assert_eq!(PackageManager::detect(dir), PackageManager::Pnpm);
        touch(dir, "bun.lockb");
        assert_eq!(PackageManager::detect(dir), PackageManager::Bun);
    }

    #[test]
    fn detect_accepts_either_bun_lockfile_name() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "bun.lock");
        assert_eq!(PackageManager::detect(tmp.path()), PackageManager::Bun);
    }

    #[test]
    fn test_commands_are_stable() {
        assert_eq!(PackageManager::Bun.test_command(), ["bun", "test"]);
        assert_eq!(PackageManager::Pnpm.test_command(), ["pnpm", "test"]);
        assert_eq!(PackageManager::Yarn.test_command(), ["yarn", "test"]);
        assert_eq!(PackageManager::Npm.test_command(), ["npm", "test"]);
    }

    #[test]
    fn cache_env_fills_are_stable() {
        assert_eq!(
            PackageManager::Bun.cache_env_fills(),
            [("BUN_INSTALL_CACHE_DIR", "{build_cache}/bun")]
        );
        assert_eq!(
            PackageManager::Pnpm.cache_env_fills(),
            [
                ("npm_config_cache", "{build_cache}/npm"),
                ("PNPM_HOME", "{build_cache}/pnpm-home"),
            ]
        );
        assert_eq!(
            PackageManager::Yarn.cache_env_fills(),
            [("YARN_CACHE_FOLDER", "{build_cache}/yarn")]
        );
        assert_eq!(
            PackageManager::Npm.cache_env_fills(),
            [("npm_config_cache", "{build_cache}/npm")]
        );
    }
}
