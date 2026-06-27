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
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A join key for source files that prefers canonical identity but degrades to a
/// lexically-normalized path when the file is not on disk.
///
/// Report aggregation joins files produced by *different* pipelines — a mutation
/// run and a separate crap scan — that may spell the same file differently
/// (`./src/x.rs` vs `src/x.rs`, relative vs absolute). When the file exists, both
/// spellings collapse to the same canonical `SourcePath`. When it does not (a
/// file deleted mid-run, or a synthetic path in tests), there is nothing to
/// canonicalize, so the key falls back to a lexically-normalized path — which
/// still makes `./src/x.rs` and `src/x.rs` equal. Either way, equal files yield
/// equal keys, which is all a join needs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileKey(PathBuf);

impl FileKey {
    /// Join key for `path`, resolved against the current directory only.
    pub fn resolve(path: &Path) -> Self {
        match SourcePath::canonical(path) {
            Some(id) => Self(id.as_path().to_path_buf()),
            None => Self(lexical(path)),
        }
    }

    /// Join key for `path`, trying the current directory first and then `root`
    /// (so a repo-relative spelling resolves the same as an absolute one).
    pub fn resolve_under(root: &Path, path: &Path) -> Self {
        match SourcePath::canonical(path).or_else(|| SourcePath::under(root, path)) {
            Some(id) => Self(id.as_path().to_path_buf()),
            None => Self(lexical(path)),
        }
    }
}

/// Lexically normalize a path without touching the filesystem: drop `.`
/// components so `./src/x.rs` and `src/x.rs` compare equal. `..` is preserved
/// (resolving it lexically would be wrong across symlinks).
fn lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
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

    #[test]
    fn file_key_joins_real_file_across_spellings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/x.rs"), "fn main() {}").unwrap();

        // An absolute spelling (from one pipeline) and a repo-relative one (from
        // another) must collapse to the same key when the file exists.
        let absolute = FileKey::resolve(&root.join("./src/x.rs"));
        let relative = FileKey::resolve_under(root, Path::new("src/x.rs"));
        assert_eq!(absolute, relative);
    }

    #[test]
    fn file_key_falls_back_to_lexical_for_missing_file() {
        // No such file on disk (synthetic test paths, or a file deleted mid-run):
        // the two spellings still collapse lexically rather than failing to join.
        let dotted = FileKey::resolve(Path::new("./src/x.rs"));
        let plain = FileKey::resolve(Path::new("src/x.rs"));
        assert_eq!(dotted, plain);

        // Distinct missing files stay distinct.
        let other = FileKey::resolve(Path::new("src/y.rs"));
        assert_ne!(plain, other);
    }
}
