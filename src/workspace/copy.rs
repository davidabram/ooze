//! Populating a copy-backend workspace: walk the repo, reflink where the
//! filesystem supports it, fall back to normal copies, and skip build/VCS
//! directories.

use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

pub(super) fn copy_repo(src: &Path, dst: &Path) -> Result<()> {
    copy_repo_with(src, dst, reflink_file)
}

fn copy_repo_with(
    src: &Path,
    dst: &Path,
    reflink: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let mut reflinked: u64 = 0;
    let mut copied: u64 = 0;

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();

        let relative = path
            .strip_prefix(src)
            .with_context(|| format!("stripping repo prefix from {}", path.display()))?;

        if should_skip(relative) {
            if entry.file_type().is_dir() {
                continue;
            }
            continue;
        }

        let target = dst.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("creating dir {}", target.display()))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent {}", parent.display()))?;
            }

            if copy_file_with(&reflink, path, &target)? {
                reflinked += 1;
            } else {
                copied += 1;
            }
        }
    }

    tracing::debug!(reflinked, copied, "copy backend: workspace populated");

    Ok(())
}

/// Copy one regular file, attempting a reflink / copy-on-write clone first.
/// Returns `true` if the file was reflinked, `false` if it fell back to a normal
/// copy. Any reflink error (unsupported filesystem, cross-device, ...) is a
/// fallback, never a failure; only the normal copy's error is surfaced.
fn copy_file_with(
    reflink: impl Fn(&Path, &Path) -> std::io::Result<()>,
    src: &Path,
    dst: &Path,
) -> Result<bool> {
    if reflink(src, dst).is_ok() {
        return Ok(true);
    }

    std::fs::copy(src, dst)
        .with_context(|| format!("copying {} -> {}", src.display(), dst.display()))?;
    Ok(false)
}

/// Clone `src` to `dst` with the Linux `FICLONE` ioctl. Fails with the raw OS
/// error (EXDEV, EOPNOTSUPP, ENOTTY, EINVAL, ...) when the filesystem doesn't
/// support reflinks; callers treat any error as "fall back to a normal copy".
#[cfg(target_os = "linux")]
fn reflink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_int, c_ulong};

    unsafe extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }

    // _IOW(0x94, 9, int) from linux/fs.h.
    const FICLONE: c_ulong = 0x4004_9409;

    let src_file = std::fs::File::open(src)?;
    let dst_file = std::fs::File::create(dst)?;

    let rc = unsafe { ioctl(dst_file.as_raw_fd(), FICLONE, src_file.as_raw_fd()) };
    let result = if rc == 0 {
        // FICLONE clones contents only; carry over the source permissions the
        // same way std::fs::copy does.
        src_file
            .metadata()
            .and_then(|m| std::fs::set_permissions(dst, m.permissions()))
    } else {
        Err(std::io::Error::last_os_error())
    };

    if result.is_err() {
        drop(dst_file);
        let _ = std::fs::remove_file(dst);
    }
    result
}

#[cfg(not(target_os = "linux"))]
fn reflink_file(_src: &Path, _dst: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

fn should_skip(relative: &Path) -> bool {
    let first = relative.components().next();

    let Some(first) = first else {
        return false;
    };

    let first = first.as_os_str().to_string_lossy();

    matches!(
        first.as_ref(),
        ".git"
            | ".ooze"
            | "target"
            | "node_modules"
            | "vendor"
            | "__pycache__"
            | ".gradle"
            | ".direnv"
            | ".idea"
            | ".vscode"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn unsupported_reflink(_src: &Path, _dst: &Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    /// Build a small fake repo with a nested source file, an ignored
    /// directory, and (on unix) an executable script.
    fn make_repo() -> TempDir {
        let repo = tempfile::tempdir().expect("temp repo");
        let root = repo.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn one() -> i32 { 1 }\n").unwrap();
        std::fs::write(root.join("README.md"), "readme\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/junk"), "junk").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(root.join("run.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        repo
    }

    #[test]
    fn copy_repo_works_without_reflink_support() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).expect("copy succeeds");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
        assert_eq!(
            std::fs::read_to_string(dst.path().join("README.md")).unwrap(),
            "readme\n"
        );
    }

    #[test]
    fn copy_repo_skips_ignored_directories() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();

        assert!(
            !dst.path().join("target").exists(),
            "target must be skipped"
        );
        assert!(!dst.path().join(".git").exists(), ".git must be skipped");
    }

    #[test]
    fn copy_repo_leaves_original_untouched() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();
        std::fs::write(dst.path().join("src/lib.rs"), "mutated").unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n",
            "mutating the workspace must not touch the source checkout"
        );
    }

    #[test]
    #[cfg(unix)]
    fn copy_repo_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo_with(repo.path(), dst.path(), unsupported_reflink).unwrap();

        let mode = std::fs::metadata(dst.path().join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "executable bit must survive the copy");
    }

    #[test]
    fn reflink_failure_falls_back_to_normal_copy() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();
        let src = repo.path().join("src/lib.rs");
        let target = dst.path().join("lib.rs");

        // Simulate a filesystem where reflink starts (creating the file) but
        // then fails, like a failing FICLONE ioctl leaving a partial dest.
        let flaky = |_s: &Path, d: &Path| -> std::io::Result<()> {
            std::fs::write(d, "partial")?;
            std::fs::remove_file(d)?;
            Err(std::io::Error::from_raw_os_error(18)) // EXDEV
        };

        let reflinked = copy_file_with(flaky, &src, &target).expect("fallback copy succeeds");
        assert!(!reflinked, "must report a normal copy, not a reflink");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
    }

    #[test]
    fn successful_reflink_skips_normal_copy() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();
        let src = repo.path().join("src/lib.rs");
        let target = dst.path().join("lib.rs");

        let fake_clone =
            |s: &Path, d: &Path| -> std::io::Result<()> { std::fs::copy(s, d).map(|_| ()) };

        let reflinked = copy_file_with(fake_clone, &src, &target).expect("clone succeeds");
        assert!(reflinked, "must report the reflink path was taken");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
    }

    /// The real reflink attempt must never break the copy backend: whether
    /// the test filesystem supports FICLONE or not, the workspace comes out
    /// identical.
    #[test]
    fn copy_repo_with_real_reflink_attempt() {
        let repo = make_repo();
        let dst = tempfile::tempdir().unwrap();

        copy_repo(repo.path(), dst.path()).expect("copy succeeds");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("src/lib.rs")).unwrap(),
            "pub fn one() -> i32 { 1 }\n"
        );
        assert!(!dst.path().join("target").exists());
    }
}
