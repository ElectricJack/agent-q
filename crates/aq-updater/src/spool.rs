//! The spool shared with the server (`<state_dir>/spool`):
//! - `requests/` (mode 2770, group 10001): the server writes one
//!   `<uuid>.json` per update request (via a `.tmp-*` dotfile and rename);
//!   the updater reads and deletes them.
//! - `status/` (mode 0755, mounted read-only into the server): the updater
//!   publishes `status.json` and the `maintenance` marker.

use crate::engine::{Engine, Halted, MaintenanceSink, UPDATE_IN_PROGRESS};
use crate::runtime::{Runtime, RuntimeError};
use crate::verify::ReleaseVerifier;
use aq_release::{HistoryEntry, Outcome, UpdateRequest, UpdateStatus};
use semver::Version;
use std::fs::{File, OpenOptions, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

/// Largest request file accepted, in bytes.
pub const REQUEST_LIMIT: u64 = 4096;
/// Group of the server container's user; it owns `requests/`.
pub const REQUESTS_GID: u32 = 10001;
pub const REQUESTS_MODE: u32 = 0o2770;
pub const STATUS_MODE: u32 = 0o755;

pub fn spool_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("spool")
}
pub fn requests_dir(state_dir: &Path) -> PathBuf {
    spool_dir(state_dir).join("requests")
}
pub fn status_dir(state_dir: &Path) -> PathBuf {
    spool_dir(state_dir).join("status")
}

fn fsync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Create the spool directories with their modes (set explicitly, so the
/// umask does not matter). `requests/` is given group 10001 only when running
/// as root; otherwise its group is left as is.
pub fn ensure_dirs(state_dir: &Path) -> std::io::Result<()> {
    let spool = spool_dir(state_dir);
    let requests = requests_dir(state_dir);
    let status = status_dir(state_dir);
    for d in [&spool, &requests, &status] {
        std::fs::create_dir_all(d)?;
    }
    std::fs::set_permissions(&spool, Permissions::from_mode(0o755))?;
    std::fs::set_permissions(&status, Permissions::from_mode(STATUS_MODE))?;
    if nix::unistd::geteuid().is_root() {
        nix::unistd::chown(
            &requests,
            None,
            Some(nix::unistd::Gid::from_raw(REQUESTS_GID)),
        )
        .map_err(std::io::Error::from)?;
    } else {
        tracing::info!(
            dir = %requests.display(),
            "not running as root: leaving the group of the requests directory as is (the server needs group {REQUESTS_GID})"
        );
    }
    // After chown, which may clear the setgid bit.
    std::fs::set_permissions(&requests, Permissions::from_mode(REQUESTS_MODE))?;
    Ok(())
}

/// A request file that was read and deleted but not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpoolReject {
    /// File name within `requests/`.
    pub file: String,
    /// Taken from the file name (`<uuid>.json`) for an oversized file; `None`
    /// when the content could not be parsed.
    pub request_id: Option<Uuid>,
    pub reason: String,
}

impl std::fmt::Display for SpoolReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request file {:?} rejected: {}", self.file, self.reason)
    }
}

enum Taken {
    Request(UpdateRequest),
    Reject(String, Option<Uuid>),
    /// A directory, symlink, FIFO or other non-regular entry.
    NotAFile,
}

/// The id a `<uuid>.json` file name carries.
fn id_from_name(name: &std::ffi::OsStr) -> Option<Uuid> {
    name.to_str()?
        .strip_suffix(".json")
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn read_one(path: &Path, id_from_name: Option<Uuid>) -> std::io::Result<Taken> {
    // O_NOFOLLOW: a symlink is never followed; O_NONBLOCK: a FIFO cannot hang
    // the open. The type is checked on the opened file.
    let f = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.raw_os_error() == Some(nix::libc::ELOOP) => return Ok(Taken::NotAFile),
        Err(e) => return Err(e),
    };
    if !f.metadata()?.is_file() {
        return Ok(Taken::NotAFile);
    }
    let mut buf = Vec::new();
    f.take(REQUEST_LIMIT + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > REQUEST_LIMIT {
        return Ok(Taken::Reject(
            format!("larger than {REQUEST_LIMIT} bytes"),
            id_from_name,
        ));
    }
    Ok(match serde_json::from_slice::<UpdateRequest>(&buf) {
        Ok(r) => Taken::Request(r),
        // Debug-escaped: the error can quote the file's content.
        Err(e) => Taken::Reject(
            format!("not a valid update request: {:?}", e.to_string()),
            id_from_name,
        ),
    })
}

/// Request entries in `dir`, oldest first (by mtime, then name). Dotfiles
/// (the server's temp files and set-aside rejects) are skipped.
fn pending(dir: &Path) -> Vec<std::ffi::OsString> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "cannot list update requests");
            return Vec::new();
        }
    };
    let mut files: Vec<(SystemTime, std::ffi::OsString)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name();
            if name.as_encoded_bytes().starts_with(b".") {
                return None;
            }
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some((mtime, name))
        })
        .collect();
    files.sort();
    files.into_iter().map(|(_, n)| n).collect()
}

/// Set a non-file entry aside as `.rejected-<name>` (ignored from then on).
/// If even that fails, remember it for this process so it is reported once.
fn set_aside(dir: &Path, name: &std::ffi::OsStr) -> bool {
    static SEEN: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
    let path = dir.join(name);
    let mut aside = std::ffi::OsString::from(".rejected-");
    aside.push(name);
    match std::fs::rename(&path, dir.join(aside)) {
        Ok(()) => true,
        Err(e) => {
            let mut seen = SEEN.lock().unwrap_or_else(|p| p.into_inner());
            if seen.contains(&path) {
                return false;
            }
            tracing::error!(path = ?path, error = %e, "cannot set aside a rejected request entry; ignoring it");
            seen.push(path);
            true
        }
    }
}

/// Read and remove one request entry. `None`: nothing to report (it is gone,
/// could not be removed, or was already reported).
fn take_one(dir: &Path, name: &std::ffi::OsStr) -> Option<Result<UpdateRequest, SpoolReject>> {
    let path = dir.join(name);
    let id = id_from_name(name);
    let reject = |reason: &str| SpoolReject {
        file: name.to_string_lossy().into_owned(),
        request_id: id,
        reason: reason.to_string(),
    };
    let meta = std::fs::symlink_metadata(&path).ok()?;
    let odd = if name.to_str().is_none() {
        Some("the file name is not UTF-8")
    } else if !meta.file_type().is_file() {
        Some("not a regular file")
    } else {
        None
    };
    // Re-checked on the opened file, in case the entry was swapped.
    let read = match odd {
        Some(_) => Ok(Taken::NotAFile),
        None => read_one(&path, id),
    };
    if let Ok(Taken::NotAFile) = read {
        return set_aside(dir, name).then(|| Err(reject(odd.unwrap_or("not a regular file"))));
    }
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::error!(path = ?path, error = %e, "cannot delete update request; ignoring it");
        }
        return None;
    }
    Some(match read {
        Ok(Taken::Request(r)) => Ok(r),
        Ok(Taken::Reject(reason, request_id)) => Err(SpoolReject {
            request_id,
            ..reject(&reason)
        }),
        Ok(Taken::NotAFile) => Err(reject("not a regular file")),
        Err(e) => Err(reject(&format!("cannot be read: {e}"))),
    })
}

/// Read and remove every request in `dir`, oldest first (by mtime, then
/// name). Dotfiles are skipped. Regular files are deleted; anything else
/// (directories, symlinks, FIFOs, names that are not UTF-8) is rejected once
/// and set aside as `.rejected-<name>`. An entry that cannot be removed is
/// skipped and logged, so it is never acted on twice.
pub fn take_requests(dir: &Path) -> Vec<Result<UpdateRequest, SpoolReject>> {
    let out: Vec<_> = pending(dir)
        .iter()
        .filter_map(|n| take_one(dir, n))
        .collect();
    sync_requests(dir, !out.is_empty());
    out
}

fn sync_requests(dir: &Path, changed: bool) {
    if changed && let Err(e) = fsync_dir(dir) {
        tracing::warn!(dir = %dir.display(), error = %e, "cannot fsync the requests directory");
    }
}

/// Write `status.json` atomically (temp file, fsync, rename, fsync dir),
/// mode 0644: the server reads it through a read-only mount.
pub fn publish_status(dir: &Path, status: &UpdateStatus) -> std::io::Result<()> {
    let tmp = dir.join(".status.json.tmp");
    let bytes = serde_json::to_vec_pretty(status).map_err(std::io::Error::other)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&tmp)?;
    f.set_permissions(Permissions::from_mode(0o644))?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, dir.join("status.json"))?;
    fsync_dir(dir)
}

/// The last published status, if there is a readable one.
pub fn read_status(dir: &Path) -> Option<UpdateStatus> {
    let bytes = std::fs::read(dir.join("status.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// `MaintenanceSink` backed by the `status/maintenance` marker file.
#[derive(Debug, Clone)]
pub struct SpoolMaintenance {
    dir: PathBuf,
}

impl SpoolMaintenance {
    /// `status_dir`: the spool's `status/` directory.
    pub fn new(status_dir: &Path) -> Self {
        SpoolMaintenance {
            dir: status_dir.to_path_buf(),
        }
    }

    pub fn marker(&self) -> PathBuf {
        self.dir.join("maintenance")
    }
}

fn failed(what: &str, path: &Path, e: std::io::Error) -> RuntimeError {
    RuntimeError::Failed(format!("cannot {what} {}: {e}", path.display()))
}

impl MaintenanceSink for SpoolMaintenance {
    fn enter(&self) -> Result<(), RuntimeError> {
        let marker = self.marker();
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o644)
            .open(&marker)
            .map_err(|e| failed("create", &marker, e))?;
        f.set_permissions(Permissions::from_mode(0o644))
            .and_then(|()| f.sync_all())
            .map_err(|e| failed("create", &marker, e))?;
        fsync_dir(&self.dir).map_err(|e| failed("fsync", &self.dir, e))
    }

    fn leave(&self) -> Result<(), RuntimeError> {
        let marker = self.marker();
        match std::fs::remove_file(&marker) {
            Ok(()) => fsync_dir(&self.dir).map_err(|e| failed("fsync", &self.dir, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(failed("remove", &marker, e)),
        }
    }
}

/// What the last install in a pass asked for and how it ended.
struct Finished {
    request_id: Uuid,
    asked: Option<Version>,
    got: Option<Version>,
    outcome: Outcome,
}

/// Install pending requests one at a time until the spool is empty or
/// `stop` is set. `stop` is checked before each request is taken, so once it
/// is set no new install starts and untaken requests stay on disk.
///
/// Malformed entries are recorded as `Rejected`. After an install finishes,
/// a queued request that repeats it (the same version, or the channel when
/// the channel offers nothing newer than what was just installed) is
/// recorded as `Rejected` instead of being installed: `update_in_progress`
/// after a success, `duplicate of request <id>, just processed (<outcome>)`
/// otherwise. Anything else is installed in turn. Requests written while an
/// install runs are picked up by the next round of this loop.
pub async fn process_requests<R: Runtime, V: ReleaseVerifier>(
    engine: &mut Engine<R, V>,
    dir: &Path,
    stop: &tokio::sync::watch::Receiver<bool>,
) -> Result<Vec<HistoryEntry>, Halted> {
    let mut out = Vec::new();
    let mut last: Option<Finished> = None;
    loop {
        let names = pending(dir);
        if names.is_empty() || *stop.borrow() {
            return Ok(out);
        }
        for name in names {
            if *stop.borrow() {
                tracing::info!(
                    "shutdown requested; leaving queued update requests for the next start"
                );
                return Ok(out);
            }
            let Some(item) = take_one(dir, &name) else {
                continue;
            };
            sync_requests(dir, true);
            let req = match item {
                Ok(r) => r,
                Err(rej) => {
                    tracing::warn!(file = ?rej.file, reason = ?rej.reason, "rejected update request");
                    out.push(engine.reject(rej.request_id, None, rej.to_string())?);
                    continue;
                }
            };
            if let Some(prev) = &last
                && repeats(engine, &req, &prev.asked, prev.got.as_ref()).await
            {
                let detail = if prev.outcome == Outcome::Succeeded {
                    UPDATE_IN_PROGRESS.to_string()
                } else {
                    format!(
                        "duplicate of request {}, just processed ({})",
                        prev.request_id,
                        outcome_name(prev.outcome)
                    )
                };
                tracing::info!(request_id = %req.id, %detail, "repeat of the update just finished; rejected");
                let to = req.version.clone();
                out.push(engine.reject(Some(req.id), to, detail)?);
                continue;
            }
            tracing::info!(request_id = %req.id, version = ?req.version, by = ?req.requested_by, "installing");
            let asked = req.version.clone();
            let request_id = req.id;
            let entry = engine.install(req).await?;
            tracing::info!(outcome = ?entry.outcome, detail = ?entry.detail, "update finished");
            last = Some(Finished {
                request_id,
                asked,
                got: entry.to.clone(),
                outcome: entry.outcome,
            });
            out.push(entry);
        }
    }
}

/// `Outcome` as it is serialized (`rolled_back`, ...).
fn outcome_name(o: Outcome) -> String {
    serde_json::to_value(o)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{o:?}"))
}

/// Does `req` ask for what the install that just finished already did?
async fn repeats<R: Runtime, V: ReleaseVerifier>(
    engine: &mut Engine<R, V>,
    req: &UpdateRequest,
    asked: &Option<Version>,
    got: Option<&Version>,
) -> bool {
    if req.version == *asked {
        return true;
    }
    match (&req.version, got) {
        (Some(v), Some(g)) => v == g,
        (Some(_), None) => false,
        // The channel: a repeat unless it now offers something newer. If the
        // channel cannot be read, install normally (which records why).
        (None, Some(g)) => match engine.fetch_channel().await {
            Ok(m) => m.version <= *g,
            Err(e) => {
                tracing::warn!(error = %e, "cannot read the channel to compare a queued request");
                false
            }
        },
        (None, None) => false,
    }
}
