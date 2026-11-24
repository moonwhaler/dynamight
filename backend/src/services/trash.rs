//! Trash for the file manager.
//!
//! An item moves to `<anchor>/.dynamight-trash/<id>/<name>`, next to a
//! sidecar `<id>.json` that records where it came from. The anchor is the
//! nearest ancestor that is an allowed root, a mount point or the top of
//! the item's filesystem, so the move is a plain rename on one filesystem.
//! The original location is stored relative to the anchor: a USB drive
//! mounted somewhere else later still restores to the right place.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

use super::path_guard::{device, free_name, is_allowed_root, is_path_allowed, rename_noreplace, TRASH_DIR};

#[derive(Debug, Serialize, Deserialize)]
struct Sidecar {
    /// Original location, relative to the anchor
    rel_path: String,
    /// Unix seconds
    deleted_at: i64,
    is_dir: bool,
    size: Option<u64>,
}

/// One item in a trash folder
#[derive(Debug, Clone, Serialize)]
pub struct TrashItem {
    pub id: String,
    /// The trash folder holding the item
    pub root: String,
    pub name: String,
    pub original_path: String,
    pub deleted_at: i64,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug)]
pub enum TrashError {
    /// No trash folder on the item's filesystem within the allowed paths
    NoAnchor,
    /// The trash folder is on another filesystem (bind mount, subvolume)
    CrossDevice,
    Io(io::Error),
}

impl TrashError {
    pub fn reason(&self) -> &'static str {
        match self {
            TrashError::NoAnchor => "no_anchor",
            TrashError::CrossDevice => "cross_device",
            TrashError::Io(_) => "io",
        }
    }
}

#[derive(Debug)]
pub enum RestoreError {
    /// Something already exists at the original location
    Conflict(PathBuf),
    NotFound,
    Io(io::Error),
}

/// The folder whose trash takes `item` (a path with a resolved parent).
pub fn find_anchor(item: &Path, allowed: &[String], mounts: &[PathBuf]) -> Option<PathBuf> {
    let dev = device(item)?;
    let mut below: Option<&Path> = None;
    for dir in item.ancestors().skip(1) {
        if !is_path_allowed(dir, allowed) {
            return None;
        }
        if device(dir) != Some(dev) {
            // `below` is the top of the item's filesystem
            return below.map(Path::to_path_buf);
        }
        if is_allowed_root(dir, allowed) || mounts.iter().any(|m| m == dir) {
            return Some(dir.to_path_buf());
        }
        below = Some(dir);
    }
    None
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Move `item` into its trash. Returns the trash folder and the new item.
pub fn move_to_trash(item: &Path, allowed: &[String], mounts: &[PathBuf]) -> Result<(PathBuf, TrashItem), TrashError> {
    let anchor = find_anchor(item, allowed, mounts).ok_or(TrashError::NoAnchor)?;
    let root = anchor.join(TRASH_DIR);
    std::fs::create_dir_all(&root).map_err(TrashError::Io)?;

    let meta = std::fs::symlink_metadata(item).map_err(TrashError::Io)?;
    let name = item.file_name().ok_or(TrashError::NoAnchor)?.to_string_lossy().to_string();
    let rel_path = item.strip_prefix(&anchor).map_err(|_| TrashError::NoAnchor)?.to_string_lossy().to_string();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let sidecar = Sidecar {
        rel_path,
        deleted_at: now_secs(),
        is_dir: meta.is_dir(),
        size: meta.is_file().then_some(meta.len()),
    };

    let holder = root.join(&id);
    let json = root.join(format!("{id}.json"));
    std::fs::create_dir(&holder).map_err(TrashError::Io)?;
    let cleanup = || {
        let _ = std::fs::remove_dir(&holder);
        let _ = std::fs::remove_file(&json);
    };
    if let Err(e) = std::fs::write(&json, serde_json::to_vec(&sidecar).unwrap_or_default()) {
        cleanup();
        return Err(TrashError::Io(e));
    }
    if let Err(e) = std::fs::rename(item, holder.join(&name)) {
        cleanup();
        return Err(if e.raw_os_error() == Some(18) { TrashError::CrossDevice } else { TrashError::Io(e) });
    }

    let item = TrashItem {
        id,
        root: root.to_string_lossy().to_string(),
        name,
        original_path: anchor.join(&sidecar.rel_path).to_string_lossy().to_string(),
        deleted_at: sidecar.deleted_at,
        is_dir: sidecar.is_dir,
        size: sidecar.size,
    };
    Ok((root, item))
}

/// A trash id is 32 hex digits; anything else never touches the disk
pub fn is_valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn read_item(root: &Path, id: &str) -> Option<TrashItem> {
    let sidecar: Sidecar = serde_json::from_slice(&std::fs::read(root.join(format!("{id}.json"))).ok()?).ok()?;
    let mut entries = std::fs::read_dir(root.join(id)).ok()?;
    let name = entries.next()?.ok()?.file_name().to_string_lossy().to_string();
    let anchor = root.parent()?;
    Some(TrashItem {
        id: id.to_string(),
        root: root.to_string_lossy().to_string(),
        name,
        original_path: anchor.join(&sidecar.rel_path).to_string_lossy().to_string(),
        deleted_at: sidecar.deleted_at,
        is_dir: sidecar.is_dir,
        size: sidecar.size,
    })
}

/// Items in one trash folder, newest first
pub fn list(root: &Path) -> Vec<TrashItem> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut items: Vec<TrashItem> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".json")?;
            if !is_valid_id(id) {
                return None;
            }
            read_item(root, id)
        })
        .collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.deleted_at));
    items
}

/// Put an item back. With `keep_both`, a taken name gets a number.
pub fn restore(root: &Path, id: &str, keep_both: bool) -> Result<PathBuf, RestoreError> {
    let item = read_item(root, id).ok_or(RestoreError::NotFound)?;
    let original = PathBuf::from(&item.original_path);
    let parent = original.parent().ok_or(RestoreError::NotFound)?;
    std::fs::create_dir_all(parent).map_err(RestoreError::Io)?;
    let target = if keep_both { free_name(parent, &item.name, item.is_dir) } else { original.clone() };
    let source = root.join(id).join(&item.name);
    match rename_noreplace(&source, &target) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(RestoreError::Conflict(original)),
        Err(e) => return Err(RestoreError::Io(e)),
    }
    let _ = std::fs::remove_dir(root.join(id));
    let _ = std::fs::remove_file(root.join(format!("{id}.json")));
    Ok(target)
}

/// Delete an item for good
pub fn purge(root: &Path, id: &str) -> io::Result<()> {
    let holder = root.join(id);
    if holder.exists() {
        std::fs::remove_dir_all(&holder)?;
    }
    match std::fs::remove_file(root.join(format!("{id}.json"))) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Delete items older than `days`. Returns how many were removed.
pub fn purge_expired(root: &Path, days: u32) -> usize {
    let cutoff = now_secs() - i64::from(days) * 86_400;
    list(root)
        .into_iter()
        .filter(|item| item.deleted_at < cutoff)
        .filter(|item| purge(root, &item.id).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let allowed = vec![dir.path().canonicalize().unwrap().to_string_lossy().to_string()];
        (dir, allowed)
    }

    #[test]
    fn anchor_is_the_allowed_root() {
        let (dir, allowed) = setup();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/f"), b"x").unwrap();
        assert_eq!(find_anchor(&root.join("a/b/f"), &allowed, &[]), Some(root.clone()));
        // A mount point below the root wins
        assert_eq!(find_anchor(&root.join("a/b/f"), &allowed, &[root.join("a")]), Some(root.join("a")));
        // Outside the allowed paths there is no anchor
        assert_eq!(find_anchor(&root.join("a/b/f"), &[root.join("a/b/c").to_string_lossy().to_string()], &[]), None);
    }

    #[test]
    fn trash_and_restore_round_trip() {
        let (dir, allowed) = setup();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        std::fs::write(root.join("docs/sub/note.txt"), b"hi").unwrap();

        let (trash, item) = move_to_trash(&root.join("docs/sub"), &allowed, &[]).unwrap();
        assert_eq!(trash, root.join(TRASH_DIR));
        assert!(!root.join("docs/sub").exists());
        let listed = list(&trash);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].original_path, root.join("docs/sub").to_string_lossy());
        assert!(listed[0].is_dir);

        // Something new took the name: conflict, then keep both
        std::fs::create_dir(root.join("docs/sub")).unwrap();
        assert!(matches!(restore(&trash, &item.id, false), Err(RestoreError::Conflict(_))));
        let target = restore(&trash, &item.id, true).unwrap();
        assert_eq!(target, root.join("docs/sub (1)"));
        assert_eq!(std::fs::read(target.join("note.txt")).unwrap(), b"hi");
        assert!(list(&trash).is_empty());
        assert_eq!(std::fs::read_dir(&trash).unwrap().count(), 0);
    }

    #[test]
    fn restore_recreates_missing_parents() {
        let (dir, allowed) = setup();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("x/y")).unwrap();
        std::fs::write(root.join("x/y/f"), b"1").unwrap();
        let (trash, item) = move_to_trash(&root.join("x/y/f"), &allowed, &[]).unwrap();
        assert_eq!(item.size, Some(1));
        std::fs::remove_dir_all(root.join("x")).unwrap();
        restore(&trash, &item.id, false).unwrap();
        assert_eq!(std::fs::read(root.join("x/y/f")).unwrap(), b"1");
    }

    #[test]
    fn restore_follows_a_moved_anchor() {
        // The sidecar stores the path relative to the anchor (a remounted drive)
        let (dir, _) = setup();
        let base = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(base.join("usb1/photos")).unwrap();
        std::fs::write(base.join("usb1/photos/p.jpg"), b"p").unwrap();
        let allowed = vec![base.join("usb1").to_string_lossy().to_string()];
        let (_, item) = move_to_trash(&base.join("usb1/photos/p.jpg"), &allowed, &[]).unwrap();
        std::fs::rename(base.join("usb1"), base.join("usb2")).unwrap();
        let trash = base.join("usb2").join(TRASH_DIR);
        assert_eq!(list(&trash)[0].original_path, base.join("usb2/photos/p.jpg").to_string_lossy());
        restore(&trash, &item.id, false).unwrap();
        assert!(base.join("usb2/photos/p.jpg").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_trashed_itself() {
        let (dir, allowed) = setup();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("target"), b"keep").unwrap();
        std::os::unix::fs::symlink(root.join("target"), root.join("link")).unwrap();
        let (trash, item) = move_to_trash(&root.join("link"), &allowed, &[]).unwrap();
        assert!(root.join("target").exists());
        purge(&trash, &item.id).unwrap();
        assert_eq!(std::fs::read(root.join("target")).unwrap(), b"keep");
        assert!(list(&trash).is_empty());
    }

    #[test]
    fn expired_items_are_purged() {
        let (dir, allowed) = setup();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("old"), b"o").unwrap();
        std::fs::write(root.join("new"), b"n").unwrap();
        let (trash, old) = move_to_trash(&root.join("old"), &allowed, &[]).unwrap();
        move_to_trash(&root.join("new"), &allowed, &[]).unwrap();
        // Age the first item by 40 days
        let json = trash.join(format!("{}.json", old.id));
        let mut sidecar: Sidecar = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
        sidecar.deleted_at -= 40 * 86_400;
        std::fs::write(&json, serde_json::to_vec(&sidecar).unwrap()).unwrap();

        assert_eq!(purge_expired(&trash, 30), 1);
        let left = list(&trash);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name, "new");
    }

    #[test]
    fn ids_are_checked() {
        assert!(is_valid_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_valid_id("../../etc"));
        assert!(!is_valid_id("0123456789abcdef0123456789abcde"));
    }
}
