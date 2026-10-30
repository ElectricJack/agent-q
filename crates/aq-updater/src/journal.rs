//! Durable updater state: `journal.json` (atomic replace), `history.jsonl`
//! (append-only) and the process lock.
//!
//! The journal is updater-private and versioned together with the binary, so
//! it uses `deny_unknown_fields` and a `journal_format` guard. It never guesses:
//! anything it cannot parse is reported as [`JournalError::Corrupt`] with the
//! raw bytes, and the operator decides.

use aq_release::{HistoryEntry, ReleaseManifest, Step};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const JOURNAL_FORMAT: u32 = 1;
const RAW_CAP: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error(
        "journal {path} is unreadable and will not be guessed at; inspect it, then fix or remove it (raw content is in the error)"
    )]
    Corrupt { path: PathBuf, raw: String },
    #[error(
        "journal {path} has journal_format {found}, this aq-updater understands {expected}; use the matching aq-updater version"
    )]
    UnsupportedFormat {
        path: PathBuf,
        found: u64,
        expected: u32,
    },
    #[error("journal I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot serialize journal: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InFlight {
    pub request_id: Uuid,
    pub target: ReleaseManifest,
    pub step: Step,
    pub backup: Option<PathBuf>,
    pub needs_restore_on_rollback: bool,
    pub started_at: DateTime<Utc>,
    pub rollback_attempted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalState {
    pub active: Option<ReleaseManifest>,
    pub previous: Option<ReleaseManifest>,
    pub current: Option<InFlight>,
}

/// On-disk shape: `JournalState` plus the format guard.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OnDisk {
    journal_format: u32,
    active: Option<ReleaseManifest>,
    previous: Option<ReleaseManifest>,
    current: Option<InFlight>,
}

#[derive(Debug)]
pub struct Journal {
    dir: PathBuf,
    state: JournalState,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> JournalError + '_ {
    move |source| JournalError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn fsync_dir(dir: &Path) -> Result<(), JournalError> {
    File::open(dir).and_then(|d| d.sync_all()).map_err(io(dir))
}

impl Journal {
    fn journal_path(&self) -> PathBuf {
        self.dir.join("journal.json")
    }
    fn history_path(&self) -> PathBuf {
        self.dir.join("history.jsonl")
    }

    /// Open the journal. A missing file is a first run (empty state); a stale
    /// `journal.json.tmp` from an interrupted write is deleted and ignored.
    pub fn open(state_dir: &Path) -> Result<Journal, JournalError> {
        if !state_dir.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o750)
                .create(state_dir)
                .map_err(io(state_dir))?;
        }
        let tmp = state_dir.join("journal.json.tmp");
        match std::fs::remove_file(&tmp) {
            Ok(()) => tracing::warn!(path = %tmp.display(), "removed stale journal temp file"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(&tmp)(e)),
        }
        let path = state_dir.join("journal.json");
        let state = match File::open(&path) {
            Ok(mut f) => {
                let mut bytes = Vec::new();
                f.read_to_end(&mut bytes).map_err(io(&path))?;
                Self::parse(&path, &bytes)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => JournalState::default(),
            Err(e) => return Err(io(&path)(e)),
        };
        Ok(Journal {
            dir: state_dir.to_path_buf(),
            state,
        })
    }

    fn parse(path: &Path, bytes: &[u8]) -> Result<JournalState, JournalError> {
        let corrupt = || JournalError::Corrupt {
            path: path.to_path_buf(),
            raw: String::from_utf8_lossy(&bytes[..bytes.len().min(RAW_CAP)]).into_owned(),
        };
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| corrupt())?;
        match value.get("journal_format").and_then(|v| v.as_u64()) {
            Some(1) => {}
            Some(found) => {
                return Err(JournalError::UnsupportedFormat {
                    path: path.to_path_buf(),
                    found,
                    expected: JOURNAL_FORMAT,
                });
            }
            None => return Err(corrupt()),
        }
        let d: OnDisk = serde_json::from_value(value).map_err(|_| corrupt())?;
        // `Step::Unknown` is read-side only; a journal holding one was not
        // written by this binary, so do not act on it.
        if d.current.as_ref().is_some_and(|c| c.step == Step::Unknown) {
            return Err(corrupt());
        }
        Ok(JournalState {
            active: d.active,
            previous: d.previous,
            current: d.current,
        })
    }

    pub fn state(&self) -> &JournalState {
        &self.state
    }

    /// Persist `state` atomically: temp file, fsync, rename, fsync directory.
    pub fn record(&mut self, state: JournalState) -> Result<(), JournalError> {
        let disk = OnDisk {
            journal_format: JOURNAL_FORMAT,
            active: state.active.clone(),
            previous: state.previous.clone(),
            current: state.current.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&disk)?;
        let tmp = self.dir.join("journal.json.tmp");
        let dst = self.journal_path();
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o640)
            .open(&tmp)
            .map_err(io(&tmp))?;
        f.write_all(&bytes).map_err(io(&tmp))?;
        f.sync_all().map_err(io(&tmp))?;
        drop(f);
        std::fs::rename(&tmp, &dst).map_err(io(&dst))?;
        fsync_dir(&self.dir)?;
        self.state = state;
        Ok(())
    }

    /// Append one history line (`O_APPEND`) and fsync it.
    pub fn append_history(&self, entry: HistoryEntry) -> Result<(), JournalError> {
        let path = self.history_path();
        let mut line = serde_json::to_vec(&entry)?;
        line.push(b'\n');
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o640)
            .open(&path)
            .map_err(io(&path))?;
        // A single write keeps the line contiguous under O_APPEND.
        f.write_all(&line).map_err(io(&path))?;
        f.sync_all().map_err(io(&path))?;
        fsync_dir(&self.dir)
    }

    /// Newest `n` entries, newest first. Unparseable lines (for example a
    /// torn trailing append) are skipped with a warning.
    pub fn recent_history(&self, n: usize) -> Vec<HistoryEntry> {
        let path = self.history_path();
        let Ok(bytes) = std::fs::read(&path) else {
            return Vec::new();
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut out = Vec::new();
        for line in text.lines().rev() {
            if out.len() >= n {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<HistoryEntry>(line) {
                Ok(e) => out.push(e),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping malformed history line")
                }
            }
        }
        out
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("another aq-updater is running (lock: {0})")]
    AlreadyLocked(PathBuf),
    #[error("cannot open lock {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Alias matching the task contract name.
pub type AlreadyLocked = LockError;

/// Holds the exclusive lock until dropped.
///
/// `fd_lock::RwLockWriteGuard` borrows its `RwLock`, so the pair is
/// self-referential. The `RwLock` lives in a heap allocation owned through a
/// raw pointer, the guard's lifetime is erased to `'static`, and `Drop`
/// releases the guard before freeing the allocation. The guard is never
/// exposed.
pub struct UpdateLock {
    guard: Option<fd_lock::RwLockWriteGuard<'static, File>>,
    lock: *mut fd_lock::RwLock<File>,
}

// The lock is only ever touched by its owner and by Drop.
unsafe impl Send for UpdateLock {}

impl std::fmt::Debug for UpdateLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UpdateLock")
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        self.guard.take();
        // SAFETY: `lock` came from Box::into_raw in `lock()` and the only
        // borrower (the guard) was dropped on the previous line.
        drop(unsafe { Box::from_raw(self.lock) });
    }
}

/// Take a non-blocking exclusive lock on `<state_dir>/lock`.
pub fn lock(state_dir: &Path) -> Result<UpdateLock, LockError> {
    let path = state_dir.join("lock");
    let ioe = |source| LockError::Io {
        path: path.clone(),
        source,
    };
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o750)
        .create(state_dir)
        .map_err(ioe)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o640)
        .open(&path)
        .map_err(ioe)?;
    let raw = Box::into_raw(Box::new(fd_lock::RwLock::new(file)));
    // SAFETY: `raw` is valid and uniquely borrowed until UpdateLock::drop.
    match unsafe { &mut *raw }.try_write() {
        Ok(guard) => Ok(UpdateLock {
            guard: Some(guard),
            lock: raw,
        }),
        Err(e) => {
            // SAFETY: the failed attempt returned no guard borrowing `raw`.
            drop(unsafe { Box::from_raw(raw) });
            if e.kind() == std::io::ErrorKind::WouldBlock {
                Err(LockError::AlreadyLocked(path))
            } else {
                Err(ioe(e))
            }
        }
    }
}
