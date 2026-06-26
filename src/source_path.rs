//! Source-file identity.
//!
//! A source file is referred to by several spellings across a run: the raw
//! `--path`-relative path a scan produced (`./src/x.rs`, `src/x.rs`), an absolute
//! path under the repo root, and the repo-relative names git prints. Comparing
//! these as plain `PathBuf`s is wrong — `./src/x.rs` and `src/x.rs` are the same
//! file but not equal strings.
//!
//! `SourcePath` is the one place that answers "are these the same file?". It is
//! an *identity*, built by canonicalizing, so every spelling of an existing file
//! collapses to one value usable as a `HashSet`/`HashMap` key. Path *display* is
//! deliberately not this type's job — keep rendering paths off the original
//! `PathBuf` so identity never depends on cosmetics.

use std::path::{Path, PathBuf};

/// The canonical identity of an on-disk source file. Two `SourcePath`s are equal
/// iff they name the same file, regardless of how the path was spelled.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourcePath(PathBuf);

impl SourcePath {
    /// Identity of a file at `path`, or `None` if it can't be canonicalized
    /// (e.g. it was deleted). Resolving requires the file to exist.
    pub fn canonical(path: &Path) -> Option<Self> {
        std::fs::canonicalize(path).ok().map(Self)
    }

    /// Identity of a repo-relative `name` resolved under `root`.
    pub fn under(root: &Path, name: &Path) -> Option<Self> {
        Self::canonical(&root.join(name))
    }

    /// The canonical absolute path backing this identity. Used by report-side
    /// aggregation (and tests) once it adopts source identity.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn differently_spelled_paths_share_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/x.rs"), "fn main() {}").unwrap();

        let plain = SourcePath::canonical(&root.join("src/x.rs")).unwrap();
        let dotted = SourcePath::canonical(&root.join("src/./x.rs")).unwrap();
        let via_under = SourcePath::under(root, Path::new("src/x.rs")).unwrap();

        assert_eq!(plain, dotted);
        assert_eq!(plain, via_under);
        // Usable as a set key: all three spellings collapse to one entry.
        let set: std::collections::HashSet<_> = [plain, dotted, via_under].into_iter().collect();
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn missing_file_has_no_identity() {
        let dir = tempfile::tempdir().unwrap();
        assert!(SourcePath::canonical(&dir.path().join("nope.rs")).is_none());
        assert!(SourcePath::under(dir.path(), Path::new("nope.rs")).is_none());
    }

    #[test]
    fn as_path_is_absolute() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.rs"), "").unwrap();
        let id = SourcePath::canonical(&dir.path().join("f.rs")).unwrap();
        assert!(id.as_path().is_absolute());
    }
}
