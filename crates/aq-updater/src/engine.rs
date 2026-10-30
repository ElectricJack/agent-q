//! Update state machine: install, bounded rollback, restore-previous, and
//! recovery from every persisted step.
//!
//! Rules the code below keeps:
//! - Each `Step` is persisted before the action it guards, so after a crash
//!   the journal alone says what may have happened.
//! - Before the replacement the active release is never stopped. Once the
//!   target has been started, a failure rolls back exactly once; a failed
//!   rollback becomes `FailedNeedsOperator` with maintenance left on.
//! - A terminal outcome is appended to history before `current` is cleared.
//!   `recover` looks for an outcome already recorded for the in-flight
//!   request, so a crash between the two never records it twice.
//! - Runtime errors marked `Interrupted` (and journal write failures) halt
//!   the engine at once with the journal as written; `recover` resumes.

use crate::config::UpdaterConfig;
use crate::journal::{InFlight, Journal, JournalError, JournalState};
use crate::runtime::{ReleaseSelector, Runtime, RuntimeError};
use crate::verify::ReleaseVerifier;
use aq_release::{
    HistoryEntry, Outcome, ReadinessReport, ReleaseManifest, ReleaseSummary, Step, UpdateRequest,
    UpdateState, UpdateStatus, check_upgrade,
};
use chrono::{DateTime, Utc};
use semver::Version;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

/// Image label carrying the release's state format; checked after pull.
pub const STATE_FORMAT_LABEL: &str = "io.agent-q.state-format";
/// Detail of a `Rejected` entry for a request that arrived mid-update.
pub const UPDATE_IN_PROGRESS: &str = "update_in_progress";
/// History entries included in `UpdateStatus`.
pub const STATUS_HISTORY: usize = 20;

/// Toggles the server-visible maintenance marker. Both calls are idempotent.
pub trait MaintenanceSink {
    fn enter(&self) -> Result<(), RuntimeError>;
    fn leave(&self) -> Result<(), RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub backup_dir: PathBuf,
    pub readiness_deadline: Duration,
    pub min_free_bytes: u64,
    pub backups_keep: usize,
}

impl EngineSettings {
    pub fn from_config(cfg: &UpdaterConfig) -> Self {
        EngineSettings {
            backup_dir: cfg.backup_dir.clone(),
            readiness_deadline: Duration::from_secs(cfg.readiness_deadline_secs),
            min_free_bytes: cfg.min_free_bytes,
            backups_keep: cfg.backups_keep,
        }
    }
}

/// The engine stopped mid-operation without reaching an outcome. The journal
/// holds the last persisted step; `Engine::recover` resumes from it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "update halted: {0}; the journal keeps the last persisted step and recovery resumes from it"
)]
pub struct Halted(pub String);

impl From<JournalError> for Halted {
    fn from(e: JournalError) -> Self {
        Halted(format!("journal write failed: {e}"))
    }
}

type Res<T> = Result<T, Halted>;
/// Inner `Err` is a failed check, with a detail for history.
type Check<T = ()> = Res<Result<T, String>>;

/// Split a runtime result into "halt now" and "this check failed".
fn rt<T>(r: Result<T, RuntimeError>) -> Check<T> {
    match r {
        Ok(v) => Ok(Ok(v)),
        Err(RuntimeError::Failed(m)) => Ok(Err(m)),
        Err(RuntimeError::Interrupted(m)) => Err(Halted(m)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Install,
    RestorePrevious,
}

fn step_name(s: Step) -> &'static str {
    match s {
        Step::Requested => "requested",
        Step::Validated => "validated",
        Step::Pulled => "pulled",
        Step::Maintenance => "maintenance",
        Step::BackedUp => "backed_up",
        Step::Replacing => "replacing",
        Step::Probing => "probing",
        Step::Active => "active",
        Step::RollingBack => "rolling_back",
        Step::Unknown => "unknown",
    }
}

fn summary(m: &ReleaseManifest) -> ReleaseSummary {
    ReleaseSummary {
        version: m.version.clone(),
        source_revision: m.source_revision.clone(),
        server_image: m.images.server.clone(),
        state_format: m.state_format,
    }
}

/// A readiness report passes only if it describes exactly `m` and the
/// database is reachable.
pub fn check_ready(r: &ReadinessReport, m: &ReleaseManifest) -> Result<(), String> {
    let mut bad = Vec::new();
    if r.version != m.version {
        bad.push(format!("version {} (expected {})", r.version, m.version));
    }
    if r.source_revision != m.source_revision {
        bad.push(format!(
            "source_revision {:?} (expected {:?})",
            r.source_revision, m.source_revision
        ));
    }
    if r.state_format != m.state_format {
        bad.push(format!(
            "state_format {} (expected {})",
            r.state_format, m.state_format
        ));
    }
    if r.database != "ok" {
        bad.push(format!("database {:?} (expected \"ok\")", r.database));
    }
    if (r.protocol_min, r.protocol_max) != (m.protocol.min, m.protocol.max) {
        bad.push(format!(
            "protocol {}..{} (expected {}..{})",
            r.protocol_min, r.protocol_max, m.protocol.min, m.protocol.max
        ));
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("readiness mismatch: {}", bad.join(", ")))
    }
}

/// `*.dump` files in `dir`, newest first. Names start with a UTC timestamp,
/// so name order is time order.
fn dumps(dir: &Path) -> Vec<PathBuf> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(dir = %dir.display(), error = %e, "cannot list backups");
            }
            return Vec::new();
        }
    };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "dump"))
        .collect();
    v.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    v
}

fn last_backup_size(dir: &Path) -> u64 {
    dumps(dir)
        .first()
        .and_then(|p| std::fs::metadata(p).ok())
        .map_or(0, |m| m.len())
}

// The async helpers below borrow only the runtime, never the engine, so the
// engine's futures stay `Send` without requiring `Sync` of its parts.

/// Backups hold the whole database: owner-only.
fn create_backup_dir(dir: &Path) -> Result<(), String> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

async fn take_backup<R: Runtime>(runtime: &R, dir: &Path, dest: &Path) -> Check<u64> {
    if let Err(e) = create_backup_dir(dir) {
        return Ok(Err(e));
    }
    if dest.exists() {
        return Ok(Err(format!("{} already exists", dest.display())));
    }
    rt(runtime.backup(dest).await)
}

/// Start the target; with `restore_first`, stop the server and restore that
/// backup before starting (restore-previous across state formats).
async fn replace<R: Runtime>(
    runtime: &R,
    target: &ReleaseManifest,
    restore_first: Option<&Path>,
) -> Check {
    if let Some(src) = restore_first {
        if let Err(e) = rt(runtime.stop_server().await)? {
            return Ok(Err(format!("stop_server: {e}")));
        }
        if let Err(e) = rt(runtime.restore(src).await)? {
            return Ok(Err(format!("restore of {}: {e}", src.display())));
        }
    }
    Ok(rt(runtime.start_server(&target.images.server).await)?
        .map_err(|e| format!("start_server: {e}")))
}

async fn probe<R: Runtime>(runtime: &R, deadline: Duration, m: &ReleaseManifest) -> Check {
    if let Err(e) = rt(runtime.wait_healthy(deadline).await)? {
        return Ok(Err(format!("wait_healthy: {e}")));
    }
    match rt(runtime.readiness().await)? {
        Ok(report) => Ok(check_ready(&report, m)),
        Err(e) => Ok(Err(format!("readiness: {e}"))),
    }
}

async fn ensure_running<R: Runtime>(runtime: &R, deadline: Duration, m: &ReleaseManifest) -> Check {
    if let Err(e) = rt(runtime.start_server(&m.images.server).await)? {
        return Ok(Err(format!("start_server: {e}")));
    }
    probe(runtime, deadline, m).await
}

/// Stop, restore the in-flight backup if needed, start `active`, probe.
async fn bring_back<R: Runtime>(
    runtime: &R,
    deadline: Duration,
    cur: &InFlight,
    active: &ReleaseManifest,
) -> Check {
    if cur.needs_restore_on_rollback {
        let Some(backup) = &cur.backup else {
            return Ok(Err("no pre-update backup is recorded to restore".into()));
        };
        if let Err(e) = rt(runtime.stop_server().await)? {
            return Ok(Err(format!("stop_server: {e}")));
        }
        if let Err(e) = rt(runtime.restore(backup).await)? {
            return Ok(Err(format!("restore of {}: {e}", backup.display())));
        }
    }
    ensure_running(runtime, deadline, active).await
}

pub struct Engine<R: Runtime, V: ReleaseVerifier> {
    journal: Journal,
    runtime: R,
    verifier: V,
    maintenance: Box<dyn MaintenanceSink + Send>,
    settings: EngineSettings,
    available: Option<ReleaseSummary>,
}

impl<R: Runtime, V: ReleaseVerifier> Engine<R, V> {
    pub fn new(
        journal: Journal,
        runtime: R,
        verifier: V,
        maintenance: Box<dyn MaintenanceSink + Send>,
        settings: EngineSettings,
    ) -> Self {
        Engine {
            journal,
            runtime,
            verifier,
            maintenance,
            settings,
            available: None,
        }
    }

    pub fn journal_state(&self) -> &JournalState {
        self.journal.state()
    }

    /// The newest compatible release seen on the channel, for `status()`.
    pub fn set_available(&mut self, available: Option<ReleaseSummary>) {
        self.available = available;
    }

    pub fn status(&self) -> UpdateStatus {
        let s = self.journal.state();
        let history = self
            .journal
            .recent_history(STATUS_HISTORY)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "cannot read update history");
                Vec::new()
            });
        let state = match &s.current {
            None => UpdateState::Idle,
            // Outside a running operation, an attempted rollback that is
            // still in flight means the rollback failed (or was cut short,
            // which recovery also turns into this state).
            Some(c) if c.rollback_attempted => UpdateState::FailedNeedsOperator {
                detail: self
                    .recorded_outcome(c.request_id)
                    .filter(|e| e.outcome == Outcome::FailedNeedsOperator)
                    .map(|e| e.detail)
                    .unwrap_or_else(|| {
                        format!(
                            "the rollback after the failed update to {} did not complete",
                            c.target.version
                        )
                    }),
            },
            Some(c) => UpdateState::InProgress {
                request_id: c.request_id,
                target: c.target.version.clone(),
                step: c.step,
                started_at: c.started_at,
            },
        };
        UpdateStatus {
            updater_version: env!("CARGO_PKG_VERSION").to_string(),
            state,
            active: s.active.as_ref().map(summary),
            previous: s.previous.as_ref().map(summary),
            available: self.available.clone(),
            history,
            updated_at: Utc::now(),
        }
    }

    // ---------------------------------------------------------------- install

    pub async fn install(&mut self, req: UpdateRequest) -> Res<HistoryEntry> {
        let started_at = Utc::now();
        let from = self.active_version();
        let entry = |outcome, detail: String| HistoryEntry {
            request_id: Some(req.id),
            from: from.clone(),
            to: req.version.clone(),
            outcome,
            detail,
            started_at,
            finished_at: Utc::now(),
        };
        if self.journal.state().current.is_some() {
            return self.log_only(entry(Outcome::Rejected, UPDATE_IN_PROGRESS.into()));
        }

        // Fetch, verify and parse: read-only, nothing to recover yet.
        let selector = match &req.version {
            Some(v) => ReleaseSelector::Version(v.clone()),
            None => ReleaseSelector::Channel,
        };
        let (manifest, bundle) = match rt(self.runtime.fetch_release(selector).await)? {
            Ok(x) => x,
            Err(e) => {
                return self.log_only(entry(
                    Outcome::Refused,
                    format!("cannot fetch the release: {e}"),
                ));
            }
        };
        if let Err(e) = self.verifier.verify(&manifest, &bundle) {
            return self.log_only(entry(Outcome::Refused, format!("verification failed: {e}")));
        }
        let target = match ReleaseManifest::from_json(&manifest) {
            Ok(m) => m,
            Err(e) => return self.log_only(entry(Outcome::Refused, e.to_string())),
        };
        if let Some(v) = &req.version
            && *v != target.version
        {
            return self.log_only(entry(
                Outcome::Refused,
                format!(
                    "requested {v} but the fetched manifest is for {}",
                    target.version
                ),
            ));
        }

        let mut cur = InFlight {
            request_id: req.id,
            target,
            step: Step::Requested,
            backup: None,
            needs_restore_on_rollback: false,
            started_at,
            rollback_attempted: false,
        };
        self.persist(&cur)?;
        let found = match rt(self.runtime.server_state_format().await)? {
            Ok(f) => f,
            Err(e) => {
                let detail = format!("cannot read the installed state format: {e}");
                return self.conclude(self.op_entry(&cur, Outcome::Refused, detail), false);
            }
        };
        match check_upgrade(self.journal.state().active.as_ref(), found, &cur.target) {
            Ok(plan) => cur.needs_restore_on_rollback = plan.needs_restore_on_rollback,
            Err(refusal) => {
                let detail = refusal.to_string();
                return self.conclude(self.op_entry(&cur, Outcome::Refused, detail), false);
            }
        }
        self.forward(cur, Mode::Install).await
    }

    // -------------------------------------------------------- restore-previous

    /// Make `previous` active again. Same state format: swap images. Different
    /// formats: needs `allow_restore`, then restores `active_backup` (losing
    /// changes made since it was taken). In `FailedNeedsOperator`, retries
    /// bringing back the active release once instead.
    pub async fn restore_previous(&mut self, allow_restore: bool) -> Res<HistoryEntry> {
        let started_at = Utc::now();
        let s = self.journal.state().clone();
        let from = self.active_version();
        if let Some(cur) = s.current {
            if cur.rollback_attempted {
                return self.retry_active(cur, allow_restore, started_at).await;
            }
            return self.log_only(HistoryEntry {
                request_id: None,
                from,
                to: None,
                outcome: Outcome::Rejected,
                detail: UPDATE_IN_PROGRESS.into(),
                started_at,
                finished_at: Utc::now(),
            });
        }
        let refuse = |to: Option<Version>, detail: String| HistoryEntry {
            request_id: None,
            from: from.clone(),
            to,
            outcome: Outcome::Refused,
            detail,
            started_at,
            finished_at: Utc::now(),
        };
        let (Some(active), Some(prev)) = (s.active, s.previous) else {
            return self.log_only(refuse(
                None,
                "no previous release is recorded; nothing to restore".into(),
            ));
        };
        let to = Some(prev.version.clone());
        let formats_differ = prev.state_format != active.state_format;
        if formats_differ {
            let Some(backup) = &s.active_backup else {
                return self.log_only(refuse(
                    to,
                    format!(
                        "{} uses state format {} but {} uses {}, and no backup of {}'s data is recorded; it cannot be restored automatically",
                        prev.version, prev.state_format, active.version, active.state_format, prev.version
                    ),
                ));
            };
            if !allow_restore {
                return self.log_only(refuse(
                    to,
                    format!(
                        "{} uses state format {} but {} uses {}, so {} cannot read the current data. Re-run with --restore-backup to restore {}, the backup taken before {} was installed; all changes made since that backup will be lost",
                        prev.version,
                        prev.state_format,
                        active.version,
                        active.state_format,
                        prev.version,
                        backup.display(),
                        active.version
                    ),
                ));
            }
        }
        let cur = InFlight {
            request_id: Uuid::new_v4(),
            target: prev,
            step: Step::Requested,
            backup: None,
            needs_restore_on_rollback: formats_differ,
            started_at,
            rollback_attempted: false,
        };
        self.persist(&cur)?;
        self.forward(cur, Mode::RestorePrevious).await
    }

    /// Operator-initiated second attempt after `FailedNeedsOperator`: bring
    /// back the active release, restoring the in-flight backup if the failed
    /// target may have changed the data.
    async fn retry_active(
        &mut self,
        cur: InFlight,
        allow_restore: bool,
        started_at: DateTime<Utc>,
    ) -> Res<HistoryEntry> {
        let refuse = |detail: String| HistoryEntry {
            // No request id: this must not read as the in-flight outcome.
            request_id: None,
            from: self.active_version(),
            to: self.active_version(),
            outcome: Outcome::Refused,
            detail,
            started_at,
            finished_at: Utc::now(),
        };
        let Some(active) = self.journal.state().active.clone() else {
            let e = refuse("no active release is recorded to restore".into());
            return self.log_only(e);
        };
        if cur.needs_restore_on_rollback && !allow_restore {
            let backup = cur
                .backup
                .as_ref()
                .map_or("<none recorded>".into(), |b| b.display().to_string());
            let e = refuse(format!(
                "the failed update to {} may have changed the data, so bringing back {} needs the backup {backup}. Re-run with --restore-backup to restore it; all changes made since that backup will be lost",
                cur.target.version, active.version
            ));
            return self.log_only(e);
        }
        match bring_back(
            &self.runtime,
            self.settings.readiness_deadline,
            &cur,
            &active,
        )
        .await?
        {
            Ok(()) => {
                let detail = format!(
                    "operator restore: {} is running again after the failed update to {}",
                    active.version, cur.target.version
                );
                self.conclude(self.op_entry(&cur, Outcome::RolledBack, detail), true)
            }
            Err(e) => {
                let detail = format!(
                    "operator restore of {} failed: {e}. {}",
                    active.version,
                    Self::operator_steps(&cur)
                );
                self.needs_operator(&cur, detail)
            }
        }
    }

    // ---------------------------------------------------------------- recover

    /// Resolve whatever the journal says was in flight. Call once at startup,
    /// before taking requests. `None`: nothing to do.
    pub async fn recover(&mut self) -> Res<Option<HistoryEntry>> {
        let Some(cur) = self.journal.state().current.clone() else {
            // Idle means maintenance is off; clear a marker a crash left.
            if let Err(e) = rt(self.maintenance.leave())? {
                tracing::warn!(error = %e, "cannot clear the maintenance marker");
            }
            return Ok(None);
        };
        if let Some(done) = self.recorded_outcome(cur.request_id) {
            if done.outcome != Outcome::FailedNeedsOperator {
                // Outcome recorded, journal not yet cleared.
                if let Err(e) = rt(self.maintenance.leave())? {
                    tracing::warn!(error = %e, "cannot clear the maintenance marker");
                }
                self.clear_current()?;
            }
            return Ok(None);
        }
        let why = format!("interrupted at step {}", step_name(cur.step));
        let entry = match (cur.step, cur.rollback_attempted) {
            (Step::Requested | Step::Validated | Step::Pulled, _) => {
                self.abort_recovered(cur, why, false).await?
            }
            (Step::Maintenance | Step::BackedUp, _) => {
                if cur.step == Step::BackedUp {
                    self.remove_backup(cur.backup.as_deref());
                }
                self.abort_recovered(cur, why, true).await?
            }
            (Step::Replacing | Step::Probing | Step::RollingBack, false) => {
                self.fail_forward(cur, why).await?
            }
            (Step::Active, _) => {
                let s = self.journal.state();
                let entry = HistoryEntry {
                    request_id: Some(cur.request_id),
                    from: s.previous.as_ref().map(|m| m.version.clone()),
                    to: Some(cur.target.version.clone()),
                    outcome: Outcome::Succeeded,
                    detail: format!(
                        "{} is active (finished by recovery: {why})",
                        cur.target.version
                    ),
                    started_at: cur.started_at,
                    finished_at: Utc::now(),
                };
                self.conclude(entry, true)?
            }
            (Step::Replacing | Step::Probing | Step::RollingBack, true) | (Step::Unknown, _) => {
                let detail = format!(
                    "{why} during the rollback from {}; the rollback is not retried automatically. {}",
                    cur.target.version,
                    Self::operator_steps(&cur)
                );
                self.needs_operator(&cur, detail)?
            }
        };
        Ok(Some(entry))
    }

    /// Abandon an update that never replaced the active release, then make
    /// sure the active release is up.
    async fn abort_recovered(
        &mut self,
        cur: InFlight,
        why: String,
        leave: bool,
    ) -> Res<HistoryEntry> {
        let mut detail = format!("{why}; update to {} abandoned", cur.target.version);
        if let Some(active) = self.journal.state().active.clone() {
            match ensure_running(&self.runtime, self.settings.readiness_deadline, &active).await? {
                Ok(()) => detail.push_str(&format!("; {} is running", active.version)),
                Err(e) => detail.push_str(&format!(
                    "; warning: {} did not pass its readiness check: {e}",
                    active.version
                )),
            }
        }
        self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), leave)
    }

    // --------------------------------------------------------- forward steps

    /// Steps `Validated` through `Active`. `cur` is persisted at `Requested`.
    async fn forward(&mut self, mut cur: InFlight, mode: Mode) -> Res<HistoryEntry> {
        // Validated: disk space for the backup.
        self.set_step(&mut cur, Step::Validated)?;
        let dir = self.settings.backup_dir.clone();
        // statvfs needs the directory to exist (first run).
        if let Err(e) = create_backup_dir(&dir) {
            return self.conclude(self.op_entry(&cur, Outcome::Refused, e), false);
        }
        let need = self
            .settings
            .min_free_bytes
            .max(last_backup_size(&dir).saturating_mul(3));
        match rt(self.runtime.free_bytes(&dir).await)? {
            Ok(free) if free >= need => {}
            Ok(free) => {
                let detail = format!(
                    "not enough free space in {}: {free} bytes free, {need} needed (the larger of min_free_bytes and 3x the last backup); free some space and retry",
                    dir.display()
                );
                return self.conclude(self.op_entry(&cur, Outcome::Refused, detail), false);
            }
            Err(e) => {
                let detail = format!("cannot check free space in {}: {e}", dir.display());
                return self.conclude(self.op_entry(&cur, Outcome::Refused, detail), false);
            }
        }

        // Pulled: download before anything stops.
        self.set_step(&mut cur, Step::Pulled)?;
        let image = cur.target.images.server.clone();
        if let Err(e) = rt(self.runtime.pull(&image).await)? {
            let detail = format!("pull of {image} failed: {e}");
            return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), false);
        }
        let want = cur.target.state_format.to_string();
        match rt(self.runtime.image_labels(&image).await)? {
            Ok(labels) if labels.get(STATE_FORMAT_LABEL) == Some(&want) => {}
            Ok(labels) => {
                let detail = format!(
                    "image {image} has label {STATE_FORMAT_LABEL}={:?} but the manifest says {want}",
                    labels.get(STATE_FORMAT_LABEL)
                );
                return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), false);
            }
            Err(e) => {
                let detail = format!("cannot read labels of {image}: {e}");
                return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), false);
            }
        }

        // Maintenance.
        self.set_step(&mut cur, Step::Maintenance)?;
        if let Err(e) = rt(self.maintenance.enter())? {
            let detail = format!("cannot enter maintenance: {e}");
            return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), true);
        }

        // BackedUp: the path is persisted before the dump starts.
        let from = self
            .active_version()
            .map_or_else(|| "none".to_string(), |v| v.to_string());
        let dest = dir.join(format!(
            "{}-{from}-{}.dump",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            cur.target.version
        ));
        cur.backup = Some(dest.clone());
        self.set_step(&mut cur, Step::BackedUp)?;
        if let Err(e) = take_backup(&self.runtime, &dir, &dest).await? {
            self.remove_backup(Some(&dest));
            let detail = format!(
                "backup failed, update abandoned before replacement{}: {e}",
                self.active_version()
                    .map_or(String::new(), |v| format!(" ({v} left running)"))
            );
            return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), true);
        }
        self.prune_backups(&dest);

        // Replacing.
        self.set_step(&mut cur, Step::Replacing)?;
        let restore_first = match mode {
            Mode::RestorePrevious if cur.needs_restore_on_rollback => {
                match self.journal.state().active_backup.clone() {
                    Some(b) => Some(b),
                    None => {
                        let check = "no backup recorded to restore".to_string();
                        return self.fail_forward(cur, check).await;
                    }
                }
            }
            _ => None,
        };
        if let Err(check) = replace(&self.runtime, &cur.target, restore_first.as_deref()).await? {
            return self.fail_forward(cur, check).await;
        }

        // Probing.
        self.set_step(&mut cur, Step::Probing)?;
        if let Err(check) =
            probe(&self.runtime, self.settings.readiness_deadline, &cur.target).await?
        {
            return self.fail_forward(cur, check).await;
        }

        // Active: swap, persisted with the step so recovery can finish.
        let mut s = self.journal.state().clone();
        let old = s.active.take();
        s.previous = old.clone();
        s.active = Some(cur.target.clone());
        s.active_backup = match mode {
            Mode::Install => cur.backup.clone(),
            // Ruling 17: nothing recorded to go forward with again.
            Mode::RestorePrevious => None,
        };
        cur.step = Step::Active;
        s.current = Some(cur.clone());
        self.journal.record(s)?;
        let detail = match mode {
            Mode::Install => format!("{} is active", cur.target.version),
            Mode::RestorePrevious if cur.needs_restore_on_rollback => format!(
                "restored previous release {} and its data backup",
                cur.target.version
            ),
            Mode::RestorePrevious => format!("restored previous release {}", cur.target.version),
        };
        let entry = HistoryEntry {
            request_id: Some(cur.request_id),
            from: old.map(|m| m.version),
            to: Some(cur.target.version.clone()),
            outcome: Outcome::Succeeded,
            detail,
            started_at: cur.started_at,
            finished_at: Utc::now(),
        };
        self.conclude(entry, true)
    }

    // ----------------------------------------------------- failure, rollback

    /// The target was (possibly) started and failed `check`.
    async fn fail_forward(&mut self, cur: InFlight, check: String) -> Res<HistoryEntry> {
        let Some(active) = self.journal.state().active.clone() else {
            // First install: nothing to roll back to.
            let mut detail = format!(
                "first install of {} failed: {check}; server stopped",
                cur.target.version
            );
            if let Err(e) = rt(self.runtime.stop_server().await)? {
                detail.push_str(&format!(" (stop_server also failed: {e})"));
            }
            return self.conclude(self.op_entry(&cur, Outcome::Aborted, detail), true);
        };
        self.rollback(cur, active, check).await
    }

    /// The single rollback attempt.
    async fn rollback(
        &mut self,
        mut cur: InFlight,
        active: ReleaseManifest,
        check: String,
    ) -> Res<HistoryEntry> {
        cur.step = Step::RollingBack;
        cur.rollback_attempted = true;
        self.persist(&cur)?;
        match bring_back(
            &self.runtime,
            self.settings.readiness_deadline,
            &cur,
            &active,
        )
        .await?
        {
            Ok(()) => {
                let restored = if cur.needs_restore_on_rollback {
                    " and its data restored from the pre-update backup"
                } else {
                    ""
                };
                let detail = format!(
                    "{} failed ({check}); rolled back to {}{restored}",
                    cur.target.version, active.version
                );
                self.conclude(self.op_entry(&cur, Outcome::RolledBack, detail), true)
            }
            Err(e) => {
                let detail = format!(
                    "{} failed ({check}); the rollback to {} also failed ({e}). {}",
                    cur.target.version,
                    active.version,
                    Self::operator_steps(&cur)
                );
                self.needs_operator(&cur, detail)
            }
        }
    }

    fn operator_steps(cur: &InFlight) -> String {
        let restore = if cur.needs_restore_on_rollback {
            format!(
                " --restore-backup (restores {})",
                cur.backup
                    .as_ref()
                    .map_or("<no backup recorded>".into(), |b| b.display().to_string())
            )
        } else {
            String::new()
        };
        format!(
            "Maintenance stays on and nothing further is tried automatically. Inspect the server and database (docker compose logs server), then run `aq-updater restore-previous{restore}` to retry bringing back the active release"
        )
    }

    // ------------------------------------------------------- journal helpers

    fn active_version(&self) -> Option<Version> {
        self.journal
            .state()
            .active
            .as_ref()
            .map(|m| m.version.clone())
    }

    fn op_entry(&self, cur: &InFlight, outcome: Outcome, detail: String) -> HistoryEntry {
        HistoryEntry {
            request_id: Some(cur.request_id),
            from: self.active_version(),
            to: Some(cur.target.version.clone()),
            outcome,
            detail,
            started_at: cur.started_at,
            finished_at: Utc::now(),
        }
    }

    fn persist(&mut self, cur: &InFlight) -> Res<()> {
        let mut s = self.journal.state().clone();
        s.current = Some(cur.clone());
        self.journal.record(s)?;
        Ok(())
    }

    fn set_step(&mut self, cur: &mut InFlight, step: Step) -> Res<()> {
        cur.step = step;
        self.persist(cur)
    }

    fn clear_current(&mut self) -> Res<()> {
        let mut s = self.journal.state().clone();
        s.current = None;
        self.journal.record(s)?;
        Ok(())
    }

    /// A history entry with no in-flight state behind it.
    fn log_only(&mut self, entry: HistoryEntry) -> Res<HistoryEntry> {
        self.journal.append_history(entry.clone())?;
        Ok(entry)
    }

    /// Terminal outcome: optionally leave maintenance, record history, then
    /// clear `current`.
    fn conclude(&mut self, mut entry: HistoryEntry, leave: bool) -> Res<HistoryEntry> {
        if leave && let Err(e) = rt(self.maintenance.leave())? {
            tracing::warn!(error = %e, "cannot leave maintenance");
            entry
                .detail
                .push_str(&format!("; warning: could not leave maintenance: {e}"));
        }
        self.journal.append_history(entry.clone())?;
        self.clear_current()?;
        Ok(entry)
    }

    /// Record `FailedNeedsOperator`; `current` and maintenance stay as they are.
    fn needs_operator(&mut self, cur: &InFlight, detail: String) -> Res<HistoryEntry> {
        let entry = self.op_entry(cur, Outcome::FailedNeedsOperator, detail);
        self.journal.append_history(entry.clone())?;
        Ok(entry)
    }

    /// The newest non-`Rejected` history entry for `request_id`, if any.
    fn recorded_outcome(&self, request_id: Uuid) -> Option<HistoryEntry> {
        match self.journal.recent_history(usize::MAX) {
            Ok(h) => h
                .into_iter()
                .find(|e| e.request_id == Some(request_id) && e.outcome != Outcome::Rejected),
            Err(e) => {
                tracing::warn!(error = %e, "cannot read update history");
                None
            }
        }
    }

    // -------------------------------------------------------- backup files

    fn remove_backup(&self, path: Option<&Path>) {
        let Some(p) = path else { return };
        if self.journal.state().active_backup.as_deref() == Some(p) {
            return;
        }
        match std::fs::remove_file(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(path = %p.display(), error = %e, "cannot remove partial backup")
            }
        }
    }

    /// Keep the newest `backups_keep` dumps; never delete `active_backup` or
    /// the in-flight backup.
    fn prune_backups(&self, in_flight: &Path) {
        let protected = self.journal.state().active_backup.as_deref();
        for p in dumps(&self.settings.backup_dir)
            .into_iter()
            .skip(self.settings.backups_keep)
        {
            if p == in_flight || Some(p.as_path()) == protected {
                continue;
            }
            if let Err(e) = std::fs::remove_file(&p) {
                tracing::warn!(path = %p.display(), error = %e, "cannot prune backup");
            }
        }
    }
}
