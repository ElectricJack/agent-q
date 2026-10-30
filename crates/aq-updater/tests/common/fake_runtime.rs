//! Scriptable in-memory `Runtime` plus a `MaintenanceSink` that shares its
//! call log, so tests can assert the exact order of runtime and maintenance
//! calls.
//!
//! Failures are keyed by method name and per-method call index (0-based,
//! reset by `clear_log`). `fail` returns `RuntimeError::Failed`; `crash`
//! returns `RuntimeError::Interrupted`, which makes the engine stop with the
//! journal as written, like a killed process.
//!
//! The fake models a host: which server image is running and which state
//! format the database holds. Starting a release whose readable formats
//! include the database's format "migrates" it to the release's format;
//! a release that cannot read the database reports `database: "error"`.
//! `backup` writes the database format into the dump and `restore` reads it
//! back, so a missing restore is observable.
#![allow(dead_code)]

use aq_release::{ReadinessReport, ReleaseManifest};
use aq_updater::engine::MaintenanceSink;
use aq_updater::runtime::{ReleaseSelector, Runtime, RuntimeError};
use async_trait::async_trait;
use semver::Version;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub const STATE_FORMAT_LABEL: &str = "io.agent-q.state-format";

#[derive(Clone)]
enum Script {
    Fail(String),
    Crash,
}

struct Inner {
    log: Vec<String>,
    counts: HashMap<String, usize>,
    scripts: HashMap<(String, usize), Script>,
    releases: HashMap<Version, (Vec<u8>, Vec<u8>)>,
    channel: Option<Version>,
    by_image: HashMap<String, ReleaseManifest>,
    labels: HashMap<String, HashMap<String, String>>,
    reports: HashMap<String, ReadinessReport>,
    free: u64,
    backup_size: usize,
    running: Option<String>,
    db_format: Option<u32>,
    maintenance: bool,
}

#[derive(Clone)]
pub struct FakeRuntime(Arc<Mutex<Inner>>);

impl Default for FakeRuntime {
    fn default() -> Self {
        FakeRuntime(Arc::new(Mutex::new(Inner {
            log: Vec::new(),
            counts: HashMap::new(),
            scripts: HashMap::new(),
            releases: HashMap::new(),
            channel: None,
            by_image: HashMap::new(),
            labels: HashMap::new(),
            reports: HashMap::new(),
            free: 100 * 1024 * 1024 * 1024,
            backup_size: 100,
            running: None,
            db_format: None,
            maintenance: false,
        })))
    }
}

impl FakeRuntime {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap()
    }

    /// Log the call and apply any script for this (method, index).
    fn call(&self, method: &str, arg: &str) -> Result<(), RuntimeError> {
        let mut i = self.inner();
        i.log.push(if arg.is_empty() {
            method.to_string()
        } else {
            format!("{method} {arg}")
        });
        let n = i.counts.entry(method.to_string()).or_insert(0);
        let idx = *n;
        *n += 1;
        match i.scripts.get(&(method.to_string(), idx)) {
            None => Ok(()),
            Some(Script::Fail(m)) => Err(RuntimeError::Failed(format!("{method}: {m}"))),
            Some(Script::Crash) => Err(RuntimeError::Interrupted(format!(
                "simulated crash in {method}"
            ))),
        }
    }

    // ---- scripting ----

    /// Make `method`'s `idx`-th call fail.
    pub fn fail(&self, method: &str, idx: usize, msg: &str) {
        self.inner()
            .scripts
            .insert((method.into(), idx), Script::Fail(msg.into()));
    }

    /// Make `method`'s `idx`-th call "crash" (RuntimeError::Interrupted).
    pub fn crash(&self, method: &str, idx: usize) {
        self.inner()
            .scripts
            .insert((method.into(), idx), Script::Crash);
    }

    /// Publish a release: fetchable by version, startable by its server image,
    /// labelled with its state format.
    pub fn add_release(&self, m: &ReleaseManifest, manifest: Vec<u8>, bundle: Vec<u8>) {
        let mut i = self.inner();
        i.releases.insert(m.version.clone(), (manifest, bundle));
        i.by_image.insert(m.images.server.clone(), m.clone());
        i.labels.insert(
            m.images.server.clone(),
            HashMap::from([(STATE_FORMAT_LABEL.to_string(), m.state_format.to_string())]),
        );
    }

    pub fn set_channel(&self, v: Version) {
        self.inner().channel = Some(v);
    }
    pub fn set_labels(&self, image: &str, labels: HashMap<String, String>) {
        self.inner().labels.insert(image.into(), labels);
    }
    /// Override the readiness report the given server image returns.
    pub fn set_report(&self, image: &str, r: ReadinessReport) {
        self.inner().reports.insert(image.into(), r);
    }
    pub fn set_free(&self, bytes: u64) {
        self.inner().free = bytes;
    }
    pub fn set_backup_size(&self, bytes: usize) {
        self.inner().backup_size = bytes;
    }
    pub fn set_db_format(&self, f: Option<u32>) {
        self.inner().db_format = f;
    }

    // ---- inspection ----

    pub fn log(&self) -> Vec<String> {
        self.inner().log.clone()
    }
    /// Forget the call log and reset per-method call indexes and scripts.
    pub fn clear_log(&self) {
        let mut i = self.inner();
        i.log.clear();
        i.counts.clear();
        i.scripts.clear();
    }
    /// Number of logged calls whose entry starts with `prefix`.
    pub fn count(&self, prefix: &str) -> usize {
        self.log().iter().filter(|c| c.starts_with(prefix)).count()
    }
    /// Index of the first logged call that starts with `prefix`.
    pub fn position(&self, prefix: &str) -> Option<usize> {
        self.log().iter().position(|c| c.starts_with(prefix))
    }
    pub fn running(&self) -> Option<String> {
        self.inner().running.clone()
    }
    pub fn db_format(&self) -> Option<u32> {
        self.inner().db_format
    }
    pub fn maintenance_on(&self) -> bool {
        self.inner().maintenance
    }
    pub fn maintenance(&self) -> FakeMaintenance {
        FakeMaintenance(self.clone())
    }
}

#[async_trait]
impl Runtime for FakeRuntime {
    async fn fetch_release(
        &self,
        selector: ReleaseSelector,
    ) -> Result<(Vec<u8>, Vec<u8>), RuntimeError> {
        let arg = match &selector {
            ReleaseSelector::Version(v) => v.to_string(),
            ReleaseSelector::Channel => "channel".into(),
        };
        self.call("fetch_release", &arg)?;
        let i = self.inner();
        let v = match selector {
            ReleaseSelector::Version(v) => v,
            ReleaseSelector::Channel => i
                .channel
                .clone()
                .ok_or_else(|| RuntimeError::Failed("no channel release".into()))?,
        };
        i.releases
            .get(&v)
            .cloned()
            .ok_or_else(|| RuntimeError::Failed(format!("release {v} not found")))
    }

    async fn pull(&self, image: &str) -> Result<(), RuntimeError> {
        self.call("pull", image)
    }

    async fn image_labels(&self, image: &str) -> Result<HashMap<String, String>, RuntimeError> {
        self.call("image_labels", image)?;
        Ok(self.inner().labels.get(image).cloned().unwrap_or_default())
    }

    async fn free_bytes(&self, path: &Path) -> Result<u64, RuntimeError> {
        self.call("free_bytes", &path.display().to_string())?;
        Ok(self.inner().free)
    }

    async fn backup(&self, dest: &Path) -> Result<u64, RuntimeError> {
        self.call("backup", &dest.display().to_string())?;
        let (fmt, size) = {
            let i = self.inner();
            (i.db_format, i.backup_size)
        };
        let mut body =
            format!("fmt:{}\n", fmt.map_or("none".into(), |f| f.to_string())).into_bytes();
        if body.len() < size {
            body.resize(size, b'.');
        }
        std::fs::write(dest, &body).map_err(|e| RuntimeError::Failed(e.to_string()))?;
        Ok(body.len() as u64)
    }

    async fn restore(&self, src: &Path) -> Result<(), RuntimeError> {
        self.call("restore", &src.display().to_string())?;
        let mut i = self.inner();
        if i.running.is_some() {
            return Err(RuntimeError::Failed(
                "restore while the server is running".into(),
            ));
        }
        let text = std::fs::read_to_string(src).map_err(|e| RuntimeError::Failed(e.to_string()))?;
        let fmt = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("fmt:"))
            .ok_or_else(|| RuntimeError::Failed("not a fake dump".into()))?;
        i.db_format = fmt.parse().ok();
        Ok(())
    }

    async fn start_server(&self, image: &str) -> Result<(), RuntimeError> {
        self.call("start_server", image)?;
        let mut i = self.inner();
        let m = i
            .by_image
            .get(image)
            .cloned()
            .ok_or_else(|| RuntimeError::Failed(format!("unknown image {image}")))?;
        i.running = Some(image.to_string());
        // The server migrates data it can read up to its own format.
        if i.db_format
            .is_none_or(|f| m.readable_state_formats.contains(&f))
        {
            i.db_format = Some(m.state_format);
        }
        Ok(())
    }

    async fn stop_server(&self) -> Result<(), RuntimeError> {
        self.call("stop_server", "")?;
        self.inner().running = None;
        Ok(())
    }

    async fn wait_healthy(&self, _deadline: Duration) -> Result<(), RuntimeError> {
        self.call("wait_healthy", "")?;
        if self.inner().running.is_some() {
            Ok(())
        } else {
            Err(RuntimeError::Failed("server is not running".into()))
        }
    }

    async fn readiness(&self) -> Result<ReadinessReport, RuntimeError> {
        self.call("readiness", "")?;
        let i = self.inner();
        let image = i
            .running
            .clone()
            .ok_or_else(|| RuntimeError::Failed("server is not running".into()))?;
        if let Some(r) = i.reports.get(&image) {
            return Ok(r.clone());
        }
        let m = &i.by_image[&image];
        let readable = i
            .db_format
            .is_some_and(|f| m.readable_state_formats.contains(&f));
        Ok(ReadinessReport {
            version: m.version.clone(),
            source_revision: m.source_revision.clone(),
            state_format: m.state_format,
            protocol_min: m.protocol.min,
            protocol_max: m.protocol.max,
            database: if readable { "ok" } else { "error" }.into(),
            auth_mode: "access".into(),
        })
    }

    async fn server_state_format(&self) -> Result<Option<u32>, RuntimeError> {
        self.call("server_state_format", "")?;
        Ok(self.inner().db_format)
    }
}

pub struct FakeMaintenance(FakeRuntime);

impl MaintenanceSink for FakeMaintenance {
    fn enter(&self) -> Result<(), RuntimeError> {
        self.0.call("enter", "")?;
        self.0.inner().maintenance = true;
        Ok(())
    }
    fn leave(&self) -> Result<(), RuntimeError> {
        self.0.call("leave", "")?;
        self.0.inner().maintenance = false;
        Ok(())
    }
}
