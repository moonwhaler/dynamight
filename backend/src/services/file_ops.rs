//! Copy and move jobs of the file manager.
//!
//! Tasks run one at a time, in the order they were created, on a blocking
//! thread. Progress goes out on a broadcast channel (at most four times a
//! second). When a target name is taken the task waits for the user's
//! choice: replace (the old item goes to the trash), keep both (a numbered
//! name) or skip, optionally for all later conflicts too.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Semaphore};
use tokio_util::sync::CancellationToken;

use super::path_guard::{device, free_name, mount_points, rename_noreplace, PART_SUFFIX};
use super::trash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Copy,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    /// Waiting for the user to decide about a taken name
    Conflict,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
pub struct Facts {
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    /// The taken target path
    pub path: String,
    pub existing: Facts,
    pub incoming: Facts,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: u64,
    /// Grows with every change: clients drop updates older than what they have
    pub revision: u64,
    pub kind: TaskKind,
    pub sources: Vec<String>,
    pub target: String,
    pub state: TaskState,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub total_files: u64,
    pub done_files: u64,
    /// The file being copied
    pub current: Option<String>,
    pub conflict: Option<Conflict>,
    pub errors: Vec<TaskError>,
    /// Set when the whole task failed
    pub failure: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

/// What the event channel carries
#[derive(Debug, Clone, Serialize)]
pub struct FileTaskEvent {
    pub task: TaskInfo,
}

struct Task {
    info: Mutex<TaskInfo>,
    cancel: CancellationToken,
    choices: Mutex<Option<mpsc::Sender<(Choice, bool)>>>,
    finished: Mutex<Option<Instant>>,
}

/// A finished task stays listed this long
const KEEP_FINISHED: Duration = Duration::from_secs(600);
const BUFFER: usize = 1024 * 1024;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

pub struct FileOps {
    tasks: Mutex<Vec<Arc<Task>>>,
    next_id: AtomicU64,
    slot: Arc<Semaphore>,
    events: broadcast::Sender<FileTaskEvent>,
    allowed: Vec<String>,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn facts(path: &Path) -> Facts {
    let meta = std::fs::symlink_metadata(path).ok();
    Facts {
        is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
        size: meta.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
        modified: meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    }
}

/// Files and bytes below `path` (links count as files of size 0)
fn measure(path: &Path) -> (u64, u64) {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_type().is_dir())
        .fold((0, 0), |(files, bytes), e| {
            let size = if e.file_type().is_file() { e.metadata().map(|m| m.len()).unwrap_or(0) } else { 0 };
            (files + 1, bytes + size)
        })
}

impl FileOps {
    pub fn new(allowed: Vec<String>) -> Arc<Self> {
        let (events, _) = broadcast::channel(256);
        Arc::new(Self {
            tasks: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            slot: Arc::new(Semaphore::new(1)),
            events,
            allowed,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileTaskEvent> {
        self.events.subscribe()
    }

    fn prune(&self) {
        self.tasks
            .lock()
            .unwrap()
            .retain(|t| t.finished.lock().unwrap().is_none_or(|at| at.elapsed() < KEEP_FINISHED));
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        self.prune();
        self.tasks.lock().unwrap().iter().map(|t| t.info.lock().unwrap().clone()).collect()
    }

    fn find(&self, id: u64) -> Option<Arc<Task>> {
        self.tasks.lock().unwrap().iter().find(|t| t.info.lock().unwrap().id == id).cloned()
    }

    pub fn cancel(&self, id: u64) -> bool {
        let Some(task) = self.find(id) else { return false };
        task.cancel.cancel();
        true
    }

    /// Answer a conflict; false if the task is not waiting for one
    pub fn resolve(&self, id: u64, choice: Choice, apply_to_all: bool) -> bool {
        let Some(task) = self.find(id) else { return false };
        let sender = task.choices.lock().unwrap().clone();
        let Some(tx) = sender else { return false };
        // Held while sending: the worker cannot report its next conflict
        // before this one is marked as answered
        let mut info = task.info.lock().unwrap();
        if info.state != TaskState::Conflict || tx.send((choice, apply_to_all)).is_err() {
            return false;
        }
        info.state = TaskState::Running;
        info.conflict = None;
        info.revision += 1;
        true
    }

    /// Queue a copy or move. Paths must already be checked by the caller:
    /// sources are entries (a link is the link), the target an existing folder.
    pub fn start(self: &Arc<Self>, kind: TaskKind, sources: Vec<PathBuf>, target: PathBuf) -> TaskInfo {
        self.prune();
        let info = TaskInfo {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            kind,
            sources: sources.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            target: target.to_string_lossy().to_string(),
            state: TaskState::Queued,
            total_bytes: 0,
            done_bytes: 0,
            total_files: 0,
            done_files: 0,
            current: None,
            conflict: None,
            errors: Vec::new(),
            failure: None,
            created_at: now_secs(),
            finished_at: None,
        };
        let task = Arc::new(Task {
            info: Mutex::new(info.clone()),
            cancel: CancellationToken::new(),
            choices: Mutex::new(None),
            finished: Mutex::new(None),
        });
        self.tasks.lock().unwrap().push(Arc::clone(&task));
        let _ = self.events.send(FileTaskEvent { task: info.clone() });

        let ops = Arc::clone(self);
        tokio::spawn(async move {
            let permit = tokio::select! {
                p = Arc::clone(&ops.slot).acquire_owned() => p.ok(),
                _ = task.cancel.cancelled() => None,
            };
            let runner = Arc::clone(&ops);
            let job = Arc::clone(&task);
            let result = match permit {
                Some(_permit) => tokio::task::spawn_blocking(move || runner.run(&job, kind, &sources, &target))
                    .await
                    .unwrap_or_else(|e| Err(io::Error::other(e.to_string()))),
                None => Err(io::Error::from(io::ErrorKind::Interrupted)),
            };
            ops.finish(&task, result);
        });
        info
    }

    fn emit(&self, task: &Task) {
        let info = {
            let mut info = task.info.lock().unwrap();
            info.revision += 1;
            info.clone()
        };
        let _ = self.events.send(FileTaskEvent { task: info });
    }

    fn finish(&self, task: &Task, result: io::Result<()>) {
        {
            let mut info = task.info.lock().unwrap();
            info.current = None;
            info.conflict = None;
            info.finished_at = Some(now_secs());
            info.state = match &result {
                _ if task.cancel.is_cancelled() => TaskState::Cancelled,
                Ok(()) => TaskState::Done,
                Err(e) => {
                    info.failure = Some(e.to_string());
                    TaskState::Failed
                }
            };
        }
        *task.finished.lock().unwrap() = Some(Instant::now());
        self.emit(task);
    }

    fn run(&self, task: &Task, kind: TaskKind, sources: &[PathBuf], target: &Path) -> io::Result<()> {
        let (files, bytes) = sources.iter().map(|s| measure(s)).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        // Copies (and moves to another drive) need the room on the target
        let copies = kind == TaskKind::Copy || sources.iter().any(|s| device(s) != device(target));
        if copies {
            if let Some((free, _)) = super::fs_space(target) {
                if free < bytes {
                    return Err(io::Error::other(format!(
                        "not enough free space on the target drive ({free} bytes free, {bytes} needed)"
                    )));
                }
            }
        }
        {
            let mut info = task.info.lock().unwrap();
            info.state = TaskState::Running;
            info.total_files = files;
            info.total_bytes = bytes;
        }
        self.emit(task);

        let (tx, rx) = mpsc::channel();
        *task.choices.lock().unwrap() = Some(tx);
        let mut run = Run {
            ops: self,
            task,
            kind,
            choices: rx,
            apply_all: None,
            last_emit: Instant::now(),
            mounts: mount_points(),
        };
        for source in sources {
            if task.cancel.is_cancelled() {
                break;
            }
            let Some(name) = source.file_name() else { continue };
            if let Err(e) = run.item(source, &target.join(name)) {
                if e.kind() == io::ErrorKind::Interrupted {
                    break;
                }
                run.error(source, &e.to_string());
            }
        }
        *task.choices.lock().unwrap() = None;
        Ok(())
    }
}

/// State of one running task
struct Run<'a> {
    ops: &'a FileOps,
    task: &'a Task,
    kind: TaskKind,
    choices: mpsc::Receiver<(Choice, bool)>,
    apply_all: Option<Choice>,
    last_emit: Instant,
    mounts: Vec<PathBuf>,
}

fn cancelled() -> io::Error {
    io::Error::from(io::ErrorKind::Interrupted)
}

impl Run<'_> {
    fn check(&self) -> io::Result<()> {
        if self.task.cancel.is_cancelled() {
            Err(cancelled())
        } else {
            Ok(())
        }
    }

    fn progress(&mut self, force: bool) {
        if force || self.last_emit.elapsed() >= PROGRESS_EVERY {
            self.last_emit = Instant::now();
            self.ops.emit(self.task);
        }
    }

    fn error(&mut self, path: &Path, message: &str) {
        self.task.info.lock().unwrap().errors.push(TaskError {
            path: path.to_string_lossy().to_string(),
            message: message.to_string(),
        });
        self.progress(true);
    }

    /// Count a file (and its bytes) as handled without copying it
    fn skip_counted(&mut self, path: &Path) {
        let (files, bytes) = measure(path);
        let mut info = self.task.info.lock().unwrap();
        info.done_files += files;
        info.done_bytes += bytes;
    }

    /// Ask the user what to do about a taken name
    fn decide(&mut self, source: &Path, dest: &Path) -> io::Result<Choice> {
        if let Some(choice) = self.apply_all {
            return Ok(choice);
        }
        {
            let mut info = self.task.info.lock().unwrap();
            info.state = TaskState::Conflict;
            info.conflict = Some(Conflict {
                path: dest.to_string_lossy().to_string(),
                existing: facts(dest),
                incoming: facts(source),
            });
        }
        self.progress(true);
        let answer = loop {
            self.check()?;
            match self.choices.recv_timeout(Duration::from_millis(200)) {
                Ok(answer) => break answer,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(cancelled()),
            }
        };
        {
            let mut info = self.task.info.lock().unwrap();
            info.state = TaskState::Running;
            info.conflict = None;
        }
        if answer.1 {
            self.apply_all = Some(answer.0);
        }
        self.progress(true);
        Ok(answer.0)
    }

    /// Where `source` goes when `dest` is taken: `None` to skip it
    fn settle(&mut self, source: &Path, dest: &Path) -> io::Result<Option<PathBuf>> {
        let source_is_dir = std::fs::symlink_metadata(source).is_ok_and(|m| m.is_dir());
        match self.decide(source, dest)? {
            Choice::Skip => Ok(None),
            Choice::KeepBoth => {
                let parent = dest.parent().unwrap_or(Path::new("/"));
                let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                Ok(Some(free_name(parent, &name, source_is_dir)))
            }
            Choice::Replace => match trash::move_to_trash(dest, &self.ops.allowed, &self.mounts) {
                Ok(_) => Ok(Some(dest.to_path_buf())),
                // Never delete for good without asking: report and leave both
                Err(_) => Err(io::Error::other("cannot replace: there is no trash on this drive")),
            },
        }
    }

    /// Copy or move one entry (file, link or folder) to `dest`
    fn item(&mut self, source: &Path, dest: &Path) -> io::Result<()> {
        self.check()?;
        let meta = std::fs::symlink_metadata(source)?;
        let dest_meta = std::fs::symlink_metadata(dest).ok();

        // Two folders: merge, the files inside raise their own conflicts
        let merge = meta.is_dir() && dest_meta.as_ref().is_some_and(|m| m.is_dir() && !m.file_type().is_symlink());
        let dest = if dest_meta.is_some() && !merge {
            match self.settle(source, dest)? {
                Some(d) => d,
                None => {
                    self.skip_counted(source);
                    self.progress(false);
                    return Ok(());
                }
            }
        } else {
            dest.to_path_buf()
        };

        if self.kind == TaskKind::Move && !merge {
            match rename_noreplace(source, &dest) {
                Ok(()) => {
                    self.skip_counted(&dest);
                    self.progress(false);
                    return Ok(());
                }
                // Another drive: copy, then remove what was copied
                Err(e) if e.raw_os_error() == Some(18) => {}
                Err(e) => return Err(e),
            }
        }

        let file_type = meta.file_type();
        if file_type.is_symlink() {
            self.link(source, &dest)?;
        } else if file_type.is_dir() {
            self.folder(source, &dest, merge)?;
        } else if file_type.is_file() {
            self.file(source, &dest, &meta)?;
        } else {
            // Sockets, devices, pipes
            self.error(source, "not a regular file");
        }
        Ok(())
    }

    fn link(&mut self, source: &Path, dest: &Path) -> io::Result<()> {
        let target = std::fs::read_link(source)?;
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&target, dest);
        #[cfg(not(unix))]
        let made: io::Result<()> = Err(io::Error::other("links are not supported"));
        match made {
            Ok(()) => {
                if self.kind == TaskKind::Move {
                    std::fs::remove_file(source)?;
                }
            }
            // vfat/exFAT cannot hold links
            Err(e) => self.error(source, &format!("could not copy the link: {e}")),
        }
        self.task.info.lock().unwrap().done_files += 1;
        self.progress(false);
        Ok(())
    }

    fn folder(&mut self, source: &Path, dest: &Path, merge: bool) -> io::Result<()> {
        if !merge {
            std::fs::create_dir(dest)?;
        }
        let mut entries: Vec<_> = std::fs::read_dir(source)?.flatten().map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            self.check()?;
            let Some(name) = entry.file_name() else { continue };
            if let Err(e) = self.item(&entry, &dest.join(name)) {
                if e.kind() == io::ErrorKind::Interrupted {
                    return Err(e);
                }
                self.error(&entry, &e.to_string());
            }
        }
        if let Ok(meta) = std::fs::metadata(source) {
            // Best effort: USB drives (vfat, exFAT) refuse permissions
            let _ = std::fs::set_permissions(dest, meta.permissions());
        }
        if self.kind == TaskKind::Move {
            // Only once everything inside moved: skipped files keep it
            let _ = std::fs::remove_dir(source);
        }
        Ok(())
    }

    fn file(&mut self, source: &Path, dest: &Path, meta: &std::fs::Metadata) -> io::Result<()> {
        let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let part = dest.with_file_name(format!(".{name}{PART_SUFFIX}"));
        self.task.info.lock().unwrap().current = Some(source.to_string_lossy().to_string());

        let copied = (|| -> io::Result<()> {
            let mut input = std::fs::File::open(source)?;
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&part)?;
            let mut buf = vec![0u8; BUFFER];
            loop {
                self.check()?;
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                output.write_all(&buf[..n])?;
                self.task.info.lock().unwrap().done_bytes += n as u64;
                self.progress(false);
            }
            output.flush()?;
            if let Ok(modified) = meta.modified() {
                let _ = output.set_modified(modified);
            }
            drop(output);
            let _ = std::fs::set_permissions(&part, meta.permissions());
            rename_noreplace(&part, dest)
        })();

        if let Err(e) = copied {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        if self.kind == TaskKind::Move {
            std::fs::remove_file(source)?;
        }
        self.task.info.lock().unwrap().done_files += 1;
        self.progress(false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, Arc<FileOps>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ops = FileOps::new(vec![root.to_string_lossy().to_string()]);
        (dir, root, ops)
    }

    async fn wait(ops: &FileOps, id: u64) -> TaskInfo {
        for _ in 0..200 {
            let info = ops.list().into_iter().find(|t| t.id == id).unwrap();
            if matches!(info.state, TaskState::Done | TaskState::Failed | TaskState::Cancelled | TaskState::Conflict) {
                return info;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("task did not finish");
    }

    #[tokio::test]
    async fn copies_a_tree_with_progress() {
        let (_d, root, ops) = setup();
        std::fs::create_dir_all(root.join("src/sub")).unwrap();
        std::fs::write(root.join("src/a.txt"), b"hello").unwrap();
        std::fs::write(root.join("src/sub/b.bin"), vec![7u8; 3 * BUFFER + 5]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("a.txt", root.join("src/link")).unwrap();
        std::fs::create_dir(root.join("dst")).unwrap();

        let info = ops.start(TaskKind::Copy, vec![root.join("src")], root.join("dst"));
        let done = wait(&ops, info.id).await;
        assert_eq!(done.state, TaskState::Done, "{:?}", done);
        assert_eq!(done.done_bytes, done.total_bytes);
        assert_eq!(std::fs::read(root.join("dst/src/a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::metadata(root.join("dst/src/sub/b.bin")).unwrap().len(), 3 * BUFFER as u64 + 5);
        #[cfg(unix)]
        assert_eq!(std::fs::read_link(root.join("dst/src/link")).unwrap(), Path::new("a.txt"));
        assert!(root.join("src/a.txt").exists());
        // No part files left behind
        assert!(!walkdir::WalkDir::new(&root).into_iter().flatten().any(|e| e.file_name().to_string_lossy().ends_with(PART_SUFFIX)));
    }

    #[tokio::test]
    async fn move_on_one_drive_renames() {
        let (_d, root, ops) = setup();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/f"), b"1").unwrap();
        std::fs::create_dir(root.join("b")).unwrap();
        let info = ops.start(TaskKind::Move, vec![root.join("a")], root.join("b"));
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Done);
        assert!(!root.join("a").exists());
        assert_eq!(std::fs::read(root.join("b/a/f")).unwrap(), b"1");
    }

    #[tokio::test]
    async fn conflicts_wait_for_a_choice() {
        let (_d, root, ops) = setup();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("dst/src")).unwrap();
        for n in ["x", "y", "z"] {
            std::fs::write(root.join("src").join(n), b"new").unwrap();
            std::fs::write(root.join("dst/src").join(n), b"old").unwrap();
        }
        std::fs::write(root.join("src/fresh"), b"fresh").unwrap();

        let info = ops.start(TaskKind::Copy, vec![root.join("src")], root.join("dst"));
        // Folders merge; the first file asks
        let waiting = wait(&ops, info.id).await;
        assert_eq!(waiting.state, TaskState::Conflict);
        assert!(waiting.conflict.as_ref().unwrap().path.ends_with("dst/src/x"));
        assert!(ops.resolve(info.id, Choice::Replace, false));
        let waiting = wait(&ops, info.id).await;
        assert!(waiting.conflict.as_ref().unwrap().path.ends_with("dst/src/y"));
        assert!(ops.resolve(info.id, Choice::KeepBoth, true));

        let done = wait(&ops, info.id).await;
        assert_eq!(done.state, TaskState::Done);
        assert_eq!(std::fs::read(root.join("dst/src/x")).unwrap(), b"new");
        assert_eq!(std::fs::read(root.join("dst/src/y")).unwrap(), b"old");
        assert_eq!(std::fs::read(root.join("dst/src/y (1)")).unwrap(), b"new");
        assert_eq!(std::fs::read(root.join("dst/src/z (1)")).unwrap(), b"new");
        assert_eq!(std::fs::read(root.join("dst/src/fresh")).unwrap(), b"fresh");
        // The replaced file is in the trash
        assert_eq!(trash::list(&root.join(".dynamight-trash")).len(), 1);
    }

    #[tokio::test]
    async fn skipped_files_stay_at_the_source_on_move() {
        let (_d, root, ops) = setup();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("dst/src")).unwrap();
        std::fs::write(root.join("src/keep"), b"mine").unwrap();
        std::fs::write(root.join("dst/src/keep"), b"theirs").unwrap();
        std::fs::write(root.join("src/go"), b"go").unwrap();

        let info = ops.start(TaskKind::Move, vec![root.join("src")], root.join("dst"));
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Conflict);
        assert!(ops.resolve(info.id, Choice::Skip, true));
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Done);
        assert_eq!(std::fs::read(root.join("src/keep")).unwrap(), b"mine");
        assert_eq!(std::fs::read(root.join("dst/src/keep")).unwrap(), b"theirs");
        assert_eq!(std::fs::read(root.join("dst/src/go")).unwrap(), b"go");
        assert!(!root.join("src/go").exists());
    }

    /// Needs two filesystems: /dev/shm and the temp dir (both tmpfs, but
    /// separate mounts on most Linux systems). Skipped when they are one.
    #[tokio::test]
    async fn move_to_another_drive_copies_then_removes() {
        let a = match tempfile::tempdir_in("/dev/shm") {
            Ok(d) => d,
            Err(_) => return,
        };
        let b = tempfile::tempdir().unwrap();
        let (ra, rb) = (a.path().canonicalize().unwrap(), b.path().canonicalize().unwrap());
        if device(&ra) == device(&rb) {
            return;
        }
        let ops = FileOps::new(vec![ra.to_string_lossy().to_string(), rb.to_string_lossy().to_string()]);
        std::fs::create_dir_all(ra.join("src/sub")).unwrap();
        std::fs::write(ra.join("src/sub/f"), b"moved").unwrap();
        std::fs::write(ra.join("src/keep"), b"mine").unwrap();
        std::fs::create_dir_all(rb.join("src")).unwrap();
        std::fs::write(rb.join("src/keep"), b"theirs").unwrap();

        let info = ops.start(TaskKind::Move, vec![ra.join("src")], rb.clone());
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Conflict);
        assert!(ops.resolve(info.id, Choice::Skip, false));
        let done = wait(&ops, info.id).await;
        assert_eq!(done.state, TaskState::Done, "{done:?}");
        assert_eq!(std::fs::read(rb.join("src/sub/f")).unwrap(), b"moved");
        // Moved files are gone at the source, the skipped one and its folder stay
        assert!(!ra.join("src/sub").exists());
        assert_eq!(std::fs::read(ra.join("src/keep")).unwrap(), b"mine");
        assert_eq!(std::fs::read(rb.join("src/keep")).unwrap(), b"theirs");
    }

    #[tokio::test]
    async fn cancel_while_waiting_ends_the_task() {
        let (_d, root, ops) = setup();
        std::fs::create_dir_all(root.join("dst")).unwrap();
        std::fs::write(root.join("f"), b"new").unwrap();
        std::fs::write(root.join("dst/f"), b"old").unwrap();
        let info = ops.start(TaskKind::Copy, vec![root.join("f")], root.join("dst"));
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Conflict);
        assert!(ops.cancel(info.id));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(wait(&ops, info.id).await.state, TaskState::Cancelled);
        assert_eq!(std::fs::read(root.join("dst/f")).unwrap(), b"old");
    }
}
