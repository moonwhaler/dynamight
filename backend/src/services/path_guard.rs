//! Path checks shared by the file endpoints: every path a client sends is
//! resolved and checked against the allowed browse paths before use.

use std::io;
use std::path::{Component, Path, PathBuf};


/// Check if a path is within one of the allowed base paths.
/// Uses canonicalization to prevent symlink bypass attacks.
pub fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> bool {
    // Handle the case where the path doesn't exist yet (for mkdir)
    // We need to check if the path or any of its parents are under allowed paths
    let path_to_check = if path.exists() {
        match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        // For non-existent paths, check the string representation
        // This is safe because we'll canonicalize during actual operations
        path.to_path_buf()
    };

    for allowed in allowed_paths {
        let allowed_path = Path::new(allowed);

        // Try to canonicalize the allowed path
        if let Ok(allowed_canonical) = allowed_path.canonicalize() {
            if path_to_check.starts_with(&allowed_canonical) {
                return true;
            }
        }

        // Also check non-canonicalized for paths that don't exist yet
        if path_to_check.starts_with(allowed) {
            return true;
        }
    }

    false
}

/// `Content-Disposition: attachment` with an ASCII fallback name and the
/// real (UTF-8) name per RFC 6266 / RFC 5987, so non-ASCII names survive.
pub fn content_disposition_attachment(filename: &str) -> String {
    let ascii: String = filename
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let encoded: String = filename
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect();
    format!("attachment; filename=\"{}\"; filename*=UTF-8''{}", ascii, encoded)
}

/// True if `path` (canonical) is one of the allowed base paths itself.
pub fn is_allowed_root(path: &Path, allowed_paths: &[String]) -> bool {
    allowed_paths.iter().any(|allowed| {
        let allowed = Path::new(allowed);
        path == allowed
            || allowed
                .canonicalize()
                .map(|c| c == path)
                .unwrap_or(false)
    })
}

/// True if `path` is a mount point or has a mount point somewhere below it.
pub fn contains_mount_point(path: &Path) -> bool {
    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    mount_targets(&mounts).iter().any(|target| Path::new(target).starts_with(path))
}

/// Parse mount targets from /proc/self/mounts content (second field, with the
/// kernel's octal escapes such as `\040` for spaces decoded).
pub fn mount_targets(mounts: &str) -> Vec<String> {
    mounts
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|raw| {
            let bytes = raw.as_bytes();
            let mut out = Vec::with_capacity(bytes.len());
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 3 < bytes.len() {
                    if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 4]).unwrap_or(""), 8) {
                        out.push(v);
                        i += 4;
                        continue;
                    }
                }
                out.push(bytes[i]);
                i += 1;
            }
            String::from_utf8_lossy(&out).into_owned()
        })
        .collect()
}

/// Folder that holds items moved to the trash, at the top of each drive
pub const TRASH_DIR: &str = ".dynamight-trash";
/// Suffix of a file still being copied (`.<name>.dynamight-part`)
pub const PART_SUFFIX: &str = ".dynamight-part";
/// Marker in the name of a file still being uploaded
pub const UPLOAD_MARKER: &str = ".dynamight-upload-";

/// Patterns that keep the file manager's own files out of backups
pub const INTERNAL_EXCLUDES: &[&str] = &[".dynamight-trash", ".*.dynamight-part", ".*.dynamight-upload-*"];

/// True for the file manager's own files, which the listing hides
pub fn is_internal_name(name: &str) -> bool {
    name == TRASH_DIR
        || (name.starts_with('.') && (name.ends_with(PART_SUFFIX) || name.contains(UPLOAD_MARKER)))
}

/// True if any component of `path` is `..`
pub fn has_parent_component(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

/// Why a client path was refused
#[derive(Debug, PartialEq, Eq)]
pub enum PathDenied {
    /// Outside the allowed paths, relative, or containing `..`
    NotAllowed,
    /// Does not exist (or cannot be resolved)
    NotFound,
}

/// Resolve an existing client path (symlinks included) to its canonical
/// form, checking the allowed paths before and after resolving.
pub fn resolve_existing(path: &str, allowed: &[String]) -> Result<PathBuf, PathDenied> {
    let p = Path::new(path);
    if !p.is_absolute() || has_parent_component(p) || !is_path_allowed(p, allowed) {
        return Err(PathDenied::NotAllowed);
    }
    let canonical = std::fs::canonicalize(p).map_err(|_| PathDenied::NotFound)?;
    if !is_path_allowed(&canonical, allowed) {
        return Err(PathDenied::NotAllowed);
    }
    Ok(canonical)
}

/// Resolve a client path whose last component is kept as is: a symlink is
/// the link itself, never its target. The parent is resolved and checked.
pub fn resolve_entry(path: &str, allowed: &[String]) -> Result<PathBuf, PathDenied> {
    let p = Path::new(path);
    if !p.is_absolute() || has_parent_component(p) {
        return Err(PathDenied::NotAllowed);
    }
    let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
        return Err(PathDenied::NotAllowed);
    };
    let parent = resolve_existing(&parent.to_string_lossy(), allowed)?;
    let entry = parent.join(name);
    if std::fs::symlink_metadata(&entry).is_err() {
        return Err(PathDenied::NotFound);
    }
    Ok(entry)
}

/// The filesystem (device id) a path is on; links are not followed
#[cfg(unix)]
pub fn device(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).ok().map(|m| m.dev())
}

#[cfg(not(unix))]
pub fn device(_path: &Path) -> Option<u64> {
    Some(0)
}

/// A name for a new or renamed file or folder: no separators, no `..`, and
/// none of the file manager's own names
pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\0'])
        && !name.contains("..")
        && name != "."
        && !is_internal_name(name)
}

/// Current mount points
pub fn mount_points() -> Vec<PathBuf> {
    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    mount_targets(&mounts).into_iter().map(PathBuf::from).collect()
}

/// Rename without ever replacing an existing target (`AlreadyExists`).
/// A change of letter case only (same file on a case-insensitive drive) is
/// allowed.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    match rename_noreplace_raw(from, to) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && same_file(from, to) => std::fs::rename(from, to),
        other => other,
    }
}

#[cfg(target_os = "linux")]
fn rename_noreplace_raw(from: &Path, to: &Path) -> io::Result<()> {
    use nix::fcntl::{renameat2, RenameFlags};
    match renameat2(None, from, None, to, RenameFlags::RENAME_NOREPLACE) {
        Ok(()) => Ok(()),
        // The filesystem does not support the flag: check, then rename
        Err(nix::errno::Errno::EINVAL) | Err(nix::errno::Errno::ENOSYS) => rename_checked(from, to),
        Err(e) => Err(io::Error::from(e)),
    }
}

#[cfg(not(target_os = "linux"))]
fn rename_noreplace_raw(from: &Path, to: &Path) -> io::Result<()> {
    rename_checked(from, to)
}

fn rename_checked(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    std::fs::rename(from, to)
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_file(_a: &Path, _b: &Path) -> bool {
    false
}

/// A name in `dir` that does not exist yet: `name`, else `name (1).ext`,
/// `name (2).ext` and so on. Folders get the number at the end.
pub fn free_name(dir: &Path, name: &str, is_dir: bool) -> PathBuf {
    let candidate = dir.join(name);
    if std::fs::symlink_metadata(&candidate).is_err() {
        return candidate;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if !is_dir && i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| std::fs::symlink_metadata(p).is_err())
        .expect("unbounded range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_names() {
        assert!(is_internal_name(".dynamight-trash"));
        assert!(is_internal_name(".video.mp4.dynamight-part"));
        assert!(is_internal_name(".video.mp4.dynamight-upload-abc"));
        assert!(!is_internal_name("dynamight-trash"));
        assert!(!is_internal_name(".bashrc"));
        assert!(!is_internal_name("notes.dynamight-part"));
    }

    #[test]
    fn names() {
        assert!(valid_name("Holiday 2026.jpg"));
        assert!(valid_name(".bashrc"));
        assert!(!valid_name(""));
        assert!(!valid_name("  "));
        assert!(!valid_name("a/b"));
        assert!(!valid_name(".."));
        assert!(!valid_name("a..b"));
        assert!(!valid_name("."));
        assert!(!valid_name(".dynamight-trash"));
        assert!(!valid_name("nul\0byte"));
    }

    #[test]
    fn parent_components() {
        assert!(has_parent_component(Path::new("/a/../b")));
        assert!(!has_parent_component(Path::new("/a/b..c/d")));
    }

    #[test]
    fn resolve_rejects_outside_and_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = vec![dir.path().to_string_lossy().to_string()];
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let f = dir.path().join("f").to_string_lossy().to_string();
        assert!(resolve_existing(&f, &allowed).is_ok());
        assert_eq!(resolve_existing("/etc/passwd", &allowed), Err(PathDenied::NotAllowed));
        let up = format!("{}/../x", dir.path().display());
        assert_eq!(resolve_existing(&up, &allowed), Err(PathDenied::NotAllowed));
        let missing = dir.path().join("nope").to_string_lossy().to_string();
        assert_eq!(resolve_existing(&missing, &allowed), Err(PathDenied::NotFound));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_entry_keeps_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = vec![dir.path().to_string_lossy().to_string()];
        let link = dir.path().join("link");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        let s = link.to_string_lossy().to_string();
        assert_eq!(resolve_entry(&s, &allowed).unwrap().file_name().unwrap(), "link");
        assert_eq!(resolve_existing(&s, &allowed), Err(PathDenied::NotAllowed));
    }

    #[test]
    fn rename_never_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let err = rename_noreplace(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&b).unwrap(), b"b");
        let c = dir.path().join("c");
        rename_noreplace(&a, &c).unwrap();
        assert!(c.exists() && !a.exists());
    }

    #[test]
    fn free_names() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(free_name(dir.path(), "a.txt", false), dir.path().join("a.txt"));
        std::fs::write(dir.path().join("a.txt"), b"").unwrap();
        std::fs::write(dir.path().join("a (1).txt"), b"").unwrap();
        assert_eq!(free_name(dir.path(), "a.txt", false), dir.path().join("a (2).txt"));
        std::fs::create_dir(dir.path().join("v1.2")).unwrap();
        assert_eq!(free_name(dir.path(), "v1.2", true), dir.path().join("v1.2 (1)"));
        std::fs::write(dir.path().join(".env"), b"").unwrap();
        assert_eq!(free_name(dir.path(), ".env", false), dir.path().join(".env (1)"));
    }
}
