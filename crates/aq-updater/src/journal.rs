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
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const JOURNAL_FORMAT: u32 = 1;
const RAW_CAP: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error(
        "journal {path} is unreadable and will not be guessed at. Restore the file by hand from the raw content in this error, status.json and history.jsonl. Do not delete it: that would lose the active, previous and in-flight release records and skip the downgrade and restore checks"
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

/// Writer handle. It borrows the [`UpdateLock`], so it cannot outlive the
/// lock and cannot be created without it. Readers use [`Journal::read`].
#[derive(Debug)]
pub struct Journal<'l> {
    dir: PathBuf,
    state: JournalState,
    _lock: PhantomData<&'l UpdateLock>,
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

impl<'l> Journal<'l> {
    fn journal_path(&self) -> PathBuf {
        self.dir.join("journal.json")
    }
    fn history_path(&self) -> PathBuf {
        self.dir.join("history.jsonl")
    }

    /// Open the journal for writing. Requires the process lock, so the stale
    /// `journal.json.tmp` cleanup cannot race a live `record`. Creates
    /// `state_dir` (0750) if missing; a missing journal is a first run.
    pub fn open(state_dir: &Path, _lock: &'l UpdateLock) -> Result<Journal<'l>, JournalError> {
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
        Ok(Journal {
            dir: state_dir.to_path_buf(),
            state: Self::read(state_dir)?,
            _lock: PhantomData,
        })
    }

    /// Pure read: no lock, no directory creation, no deletion. A missing
    /// directory or file is an empty state.
    pub fn read(state_dir: &Path) -> Result<JournalState, JournalError> {
        let path = state_dir.join("journal.json");
        match std::fs::read(&path) {
            Ok(bytes) => Self::parse(&path, &bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(JournalState::default()),
            Err(e) => Err(io(&path)(e)),
        }
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
        let manifests = [d.active.as_ref(), d.previous.as_ref()]
            .into_iter()
            .flatten()
            .chain(d.current.as_ref().map(|c| &c.target));
        for m in manifests {
            if m.validate().is_err() {
                return Err(corrupt());
            }
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

    /// Append one history line (`O_APPEND`) and fsync it. If the file ends
    /// in a torn line (no trailing newline) a newline is written first so the
    /// new entry is not glued onto the garbage.
    pub fn append_history(&self, entry: HistoryEntry) -> Result<(), JournalError> {
        let path = self.history_path();
        let mut line = serde_json::to_vec(&entry)?;
        line.push(b'\n');
        let mut f = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .mode(0o640)
            .open(&path)
            .map_err(io(&path))?;
        let len = f.metadata().map_err(io(&path))?.len();
        if len > 0 {
            f.seek(SeekFrom::Start(len - 1)).map_err(io(&path))?;
            let mut last = [0u8; 1];
            f.read_exact(&mut last).map_err(io(&path))?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        // A single write keeps the entry contiguous under O_APPEND.
        f.write_all(&line).map_err(io(&path))?;
        f.sync_all().map_err(io(&path))?;
        fsync_dir(&self.dir)
    }

    pub fn recent_history(&self, n: usize) -> Result<Vec<HistoryEntry>, JournalError> {
        read_history(&self.dir, n)
    }
}

/// Newest `n` history entries, newest first; usable without the lock.
/// A missing file is empty. Unparseable lines (for example a torn trailing
/// append) are skipped with a warning; other read errors are returned.
pub fn read_history(state_dir: &Path, n: usize) -> Result<Vec<HistoryEntry>, JournalError> {
    let path = state_dir.join("history.jsonl");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(&path)(e)),
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
    Ok(out)
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

/// Exclusive process lock; released when dropped (the OS also releases it if
/// the process dies). Locks are per open file description, so two `lock`
/// calls conflict even within one process.
#[derive(Debug)]
pub struct UpdateLock {
    _file: File,
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
    match file.try_lock() {
        Ok(()) => Ok(UpdateLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(LockError::AlreadyLocked(path)),
        Err(TryLockError::Error(e)) => Err(ioe(e)),
    }
}
