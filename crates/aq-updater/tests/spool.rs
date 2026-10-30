//! Spool intake, status publishing and the maintenance marker. The tests at
//! the bottom drive requests through the engine with `FakeRuntime` and need
//! `--features test-verifier`.

mod common;

use aq_release::{UpdateRequest, UpdateState, UpdateStatus};
use aq_updater::engine::MaintenanceSink;
use aq_updater::spool::{
    REQUEST_LIMIT, SpoolMaintenance, SpoolReject, publish_status, take_requests,
};
use chrono::Utc;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

fn request(version: Option<&str>) -> UpdateRequest {
    UpdateRequest {
        id: Uuid::new_v4(),
        version: version.map(|v| v.parse().unwrap()),
        requested_by: "owner@example.com".into(),
        requested_at: Utc::now(),
    }
}

/// Write a request the way the server does, with an explicit mtime.
fn drop_request(dir: &Path, r: &UpdateRequest, age_secs: u64) {
    let path = dir.join(format!("{}.json", r.id));
    std::fs::write(&path, serde_json::to_vec(r).unwrap()).unwrap();
    set_age(&path, age_secs);
}

fn set_age(path: &Path, age_secs: u64) {
    let f = std::fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(age_secs))
        .unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn status() -> UpdateStatus {
    UpdateStatus {
        updater_version: "0.1.0".into(),
        state: UpdateState::Idle,
        active: None,
        previous: None,
        available: None,
        history: Vec::new(),
        updated_at: Utc::now(),
    }
}

#[test]
fn take_requests_rejects_oversized_and_malformed() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path();
    let good = request(Some("1.2.3"));
    drop_request(dir, &good, 30);

    // Oversized: the id still comes from the file name.
    let big_id = Uuid::new_v4();
    let big = dir.join(format!("{big_id}.json"));
    std::fs::write(&big, vec![b' '; REQUEST_LIMIT as usize + 1]).unwrap();
    set_age(&big, 20);

    // Not JSON, and JSON with an unknown field: no id is trusted.
    let junk_id = Uuid::new_v4();
    let junk = dir.join(format!("{junk_id}.json"));
    std::fs::write(&junk, b"{not json").unwrap();
    set_age(&junk, 10);
    let extra = dir.join("extra.json");
    let mut v = serde_json::to_value(request(None)).unwrap();
    v["image"] = "evil@sha256:00".into();
    std::fs::write(&extra, serde_json::to_vec(&v).unwrap()).unwrap();
    set_age(&extra, 5);

    // Exactly at the limit is accepted (padded with whitespace).
    let edge = request(Some("2.0.0"));
    let mut body = serde_json::to_vec(&edge).unwrap();
    body.resize(REQUEST_LIMIT as usize, b' ');
    let edge_path = dir.join(format!("{}.json", edge.id));
    std::fs::write(&edge_path, &body).unwrap();
    set_age(&edge_path, 1);

    let got = take_requests(dir);
    assert_eq!(got.len(), 5, "{got:?}");
    assert_eq!(got[0], Ok(good));
    let big_rej: &SpoolReject = got[1].as_ref().unwrap_err();
    assert_eq!(big_rej.request_id, Some(big_id));
    assert!(big_rej.reason.contains("4096"), "{}", big_rej.reason);
    // Malformed content keeps the id a `<uuid>.json` name carries.
    for (r, id) in got[2..4].iter().zip([Some(junk_id), None]) {
        let rej = r.as_ref().unwrap_err();
        assert_eq!(rej.request_id, id, "{rej:?}");
        assert!(rej.reason.contains("not a valid update request"), "{rej:?}");
    }
    assert_eq!(got[4], Ok(edge));
    assert!(names(dir).is_empty(), "{:?}", names(dir));
}

#[test]
fn take_requests_rejects_non_regular_files() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path();
    let target = t.path().join("..").join("outside");
    std::os::unix::fs::symlink(&target, dir.join("link.json")).unwrap();
    nix::unistd::mkfifo(&dir.join("fifo.json"), nix::sys::stat::Mode::S_IRWXU).unwrap();
    let sub_id = Uuid::new_v4();
    std::fs::create_dir(dir.join(format!("{sub_id}.json"))).unwrap();
    std::fs::write(dir.join("inner"), b"x").unwrap();
    std::fs::rename(dir.join("inner"), dir.join(format!("{sub_id}.json/inner"))).unwrap();
    // A name that is not UTF-8, with valid content.
    use std::os::unix::ffi::OsStrExt;
    let odd = std::ffi::OsStr::from_bytes(b"req-\xff.json");
    std::fs::write(dir.join(odd), serde_json::to_vec(&request(None)).unwrap()).unwrap();

    let got = take_requests(dir);
    assert_eq!(got.len(), 4, "{got:?}");
    assert!(got.iter().all(|r| r.is_err()), "{got:?}");
    let sub = got
        .iter()
        .map(|r| r.as_ref().unwrap_err())
        .find(|r| r.file.contains(&sub_id.to_string()))
        .unwrap();
    assert_eq!(sub.request_id, Some(sub_id));
    // Recorded once, then set aside as dotfiles so nothing retries them.
    let left = names(dir);
    assert_eq!(left.len(), 4, "{left:?}");
    assert!(left.iter().all(|n| n.starts_with(".rejected-")), "{left:?}");
    assert!(take_requests(dir).is_empty());
    assert!(dir.join(format!(".rejected-{sub_id}.json/inner")).exists());
}

#[test]
fn take_requests_deletes_processed() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path();
    let (old, new, same_a, same_b) = (
        request(Some("1.0.0")),
        request(None),
        request(Some("1.0.1")),
        request(Some("1.0.2")),
    );
    drop_request(dir, &new, 10);
    drop_request(dir, &old, 100);
    // Equal mtimes fall back to name order.
    let mtime = SystemTime::now() - Duration::from_secs(50);
    for r in [&same_a, &same_b] {
        let p = dir.join(format!("{}.json", r.id));
        std::fs::write(&p, serde_json::to_vec(r).unwrap()).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
    }
    // The server's in-progress temp files are dotfiles and are left alone.
    std::fs::write(dir.join(format!(".tmp-{}", Uuid::new_v4())), b"partial").unwrap();

    let got: Vec<UpdateRequest> = take_requests(dir).into_iter().map(Result::unwrap).collect();
    let mut tie = [same_a.clone(), same_b.clone()];
    tie.sort_by_key(|r| format!("{}.json", r.id));
    assert_eq!(got, vec![old, tie[0].clone(), tie[1].clone(), new]);
    let left = names(dir);
    assert_eq!(left.len(), 1);
    assert!(left[0].starts_with(".tmp-"), "{left:?}");
    assert!(take_requests(dir).is_empty());
}

#[test]
fn publish_status_atomic() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path();
    // A temp file left by a crash does not get in the way.
    std::fs::write(dir.join(".status.json.tmp"), b"torn").unwrap();
    let mut s = status();
    publish_status(dir, &s).unwrap();
    let path = dir.join("status.json");
    let read: UpdateStatus = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(read, s);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o644, "{mode:o}");

    s.state = UpdateState::FailedNeedsOperator { detail: "x".into() };
    publish_status(dir, &s).unwrap();
    let read: UpdateStatus = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(read, s);
    assert_eq!(names(dir), vec!["status.json".to_string()]);
}

#[test]
fn maintenance_marker_toggles() {
    let t = tempfile::tempdir().unwrap();
    let m = SpoolMaintenance::new(t.path());
    let marker = t.path().join("maintenance");
    m.leave().unwrap();
    assert!(!marker.exists());
    m.enter().unwrap();
    assert!(marker.exists());
    let mode = std::fs::metadata(&marker).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o644, "{mode:o}");
    m.enter().unwrap();
    assert!(marker.exists());
    m.leave().unwrap();
    assert!(!marker.exists());
    m.leave().unwrap();
    // A failure is reported, not ignored.
    let gone = SpoolMaintenance::new(&t.path().join("missing"));
    assert!(gone.enter().is_err());
}

#[test]
fn ensure_dirs_sets_modes() {
    let t = tempfile::tempdir().unwrap();
    let state = t.path().join("state");
    aq_updater::spool::ensure_dirs(&state).unwrap();
    aq_updater::spool::ensure_dirs(&state).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(&state.join("spool/requests")), 0o2770);
    assert_eq!(mode(&state.join("spool/status")), 0o755);
}

#[cfg(feature = "test-verifier")]
mod with_engine {
    use super::*;
    use crate::common::fake_runtime::FakeRuntime;
    use aq_release::{HistoryEntry, ImageRefs, Outcome, ProtocolRange, ReleaseManifest, Step};
    use aq_updater::engine::{Engine, EngineSettings, UPDATE_IN_PROGRESS};
    use aq_updater::journal::{Journal, lock, read_history};
    use aq_updater::spool::process_requests;
    use aq_updater::verify::TestKey;
    use ed25519_dalek::{Signer, SigningKey};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    struct H {
        _t: tempfile::TempDir,
        state_dir: PathBuf,
        backup_dir: PathBuf,
        requests: PathBuf,
        fake: FakeRuntime,
        key: SigningKey,
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn manifest(v: &str) -> ReleaseManifest {
        let digits = v.replace('.', "");
        ReleaseManifest {
            manifest_version: 1,
            version: v.parse().unwrap(),
            source_revision: format!("rev-{v}"),
            images: ImageRefs {
                server: format!("ghcr.io/electricjack/agent-q-server@sha256:{digits:0>64}"),
                worker: format!("ghcr.io/electricjack/agent-q-worker@sha256:{digits:f>64}"),
            },
            protocol: ProtocolRange { min: 1, max: 1 },
            state_format: 1,
            readable_state_formats: vec![1],
            upgrade_from: ">=0.0.0".parse().unwrap(),
            created_at: Utc::now(),
        }
    }

    impl H {
        fn new() -> H {
            let t = tempfile::tempdir().unwrap();
            let requests = t.path().join("requests");
            std::fs::create_dir(&requests).unwrap();
            H {
                state_dir: t.path().join("state"),
                backup_dir: t.path().join("backups"),
                requests,
                _t: t,
                fake: FakeRuntime::default(),
                key: SigningKey::from_bytes(&[9; 32]),
            }
        }

        fn engine(&self) -> Engine<FakeRuntime, TestKey> {
            let journal = Journal::open(lock(&self.state_dir).unwrap()).unwrap();
            let verifier = TestKey::from_hex(&hex(self.key.verifying_key().as_bytes())).unwrap();
            Engine::new(
                journal,
                self.fake.clone(),
                verifier,
                Box::new(self.fake.maintenance()),
                EngineSettings {
                    backup_dir: self.backup_dir.clone(),
                    readiness_deadline: Duration::from_secs(120),
                    min_free_bytes: 1024,
                    backups_keep: 5,
                    expected_auth_mode: "cloudflare_access".into(),
                },
            )
        }

        fn publish(&self, m: &ReleaseManifest) {
            let bytes = m.to_canonical_json();
            let sig = hex(&self.key.sign(&bytes).to_bytes());
            self.fake.add_release(m, bytes, sig.into_bytes());
        }

        fn history(&self) -> Vec<HistoryEntry> {
            let mut h = read_history(&self.state_dir, 1000).unwrap();
            h.reverse();
            h
        }
    }

    fn no_stop() -> tokio::sync::watch::Receiver<bool> {
        tokio::sync::watch::channel(false).1
    }

    /// Shutdown requested while an install runs: that install finishes, the
    /// next queued request is not started and stays on disk.
    #[tokio::test]
    async fn shutdown_stops_before_the_next_install() {
        let h = H::new();
        let (a, b) = (manifest("1.0.0"), manifest("1.1.0"));
        h.publish(&a);
        h.publish(&b);
        let mut e = h.engine();
        let (first, second) = (request(Some("1.0.0")), request(Some("1.1.0")));
        drop_request(&h.requests, &first, 20);
        drop_request(&h.requests, &second, 10);
        let (tx, rx) = tokio::sync::watch::channel(false);
        e.set_observer(Box::new(move |s: &UpdateStatus| {
            if matches!(
                s.state,
                UpdateState::InProgress {
                    step: Step::Pulled,
                    ..
                }
            ) {
                let _ = tx.send(true);
            }
        }));
        let out = process_requests(&mut e, &h.requests, &rx).await.unwrap();
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].request_id, Some(first.id));
        assert_eq!(out[0].outcome, Outcome::Succeeded, "{}", out[0].detail);
        assert_eq!(h.fake.count("start_server"), 1);
        assert_eq!(names(&h.requests), vec![format!("{}.json", second.id)]);
        // Already stopped: nothing is taken at all.
        let out = process_requests(&mut e, &h.requests, &rx).await.unwrap();
        assert!(out.is_empty());
        assert_eq!(names(&h.requests).len(), 1);
    }

    /// A repeat of a request that just failed says so, rather than claiming
    /// an update is in progress.
    #[tokio::test]
    async fn repeat_after_a_failed_install_names_the_duplicate() {
        let h = H::new();
        let mut e = h.engine();
        let (first, second) = (request(Some("9.9.9")), request(Some("9.9.9")));
        drop_request(&h.requests, &first, 20);
        drop_request(&h.requests, &second, 10);
        let out = process_requests(&mut e, &h.requests, &no_stop())
            .await
            .unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].outcome, Outcome::Refused, "{}", out[0].detail);
        assert_eq!(out[1].outcome, Outcome::Rejected);
        assert_eq!(
            out[1].detail,
            format!(
                "duplicate of request {}, just processed (refused)",
                first.id
            )
        );
    }

    #[tokio::test]
    async fn malformed_requests_are_recorded_as_rejected() {
        let h = H::new();
        let mut e = h.engine();
        let big_id = Uuid::new_v4();
        std::fs::write(
            h.requests.join(format!("{big_id}.json")),
            vec![b'x'; REQUEST_LIMIT as usize + 10],
        )
        .unwrap();
        std::fs::write(h.requests.join("junk.json"), b"[]").unwrap();
        let out = process_requests(&mut e, &h.requests, &no_stop())
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        let hist = h.history();
        assert_eq!(hist, out);
        assert!(hist.iter().all(|e| e.outcome == Outcome::Rejected));
        let ids: Vec<_> = hist.iter().map(|e| e.request_id).collect();
        assert!(
            ids.contains(&Some(big_id)) && ids.contains(&None),
            "{ids:?}"
        );
        assert!(names(&h.requests).is_empty());
        assert_eq!(h.fake.log(), Vec::<String>::new(), "no runtime calls");
    }

    #[tokio::test]
    async fn duplicate_requests_in_one_batch_install_once() {
        let h = H::new();
        let a = manifest("1.0.0");
        h.publish(&a);
        let mut e = h.engine();
        let first = request(Some("1.0.0"));
        let second = request(Some("1.0.0"));
        drop_request(&h.requests, &first, 20);
        drop_request(&h.requests, &second, 10);
        let out = process_requests(&mut e, &h.requests, &no_stop())
            .await
            .unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].request_id, Some(first.id));
        assert_eq!(out[0].outcome, Outcome::Succeeded, "{}", out[0].detail);
        assert_eq!(out[1].request_id, Some(second.id));
        assert_eq!(out[1].outcome, Outcome::Rejected);
        assert_eq!(out[1].detail, UPDATE_IN_PROGRESS);
        assert_eq!(h.fake.count("start_server"), 1);
        assert_eq!(h.history(), out);
    }

    /// Requests written while an install runs: a repeat of the same target
    /// (by version, or by channel with nothing newer) is rejected; a request
    /// for another version is installed afterwards.
    #[tokio::test]
    async fn requests_arriving_mid_install_are_handled_after_it() {
        let h = H::new();
        let (a, b) = (manifest("1.0.0"), manifest("1.1.0"));
        h.publish(&a);
        h.publish(&b);
        h.fake.set_channel(a.version.clone());
        let mut e = h.engine();
        let first = request(Some("1.0.0"));
        let dup = request(Some("1.0.0"));
        let chan = request(None);
        let other = request(Some("1.1.0"));
        let late = [dup.clone(), chan.clone(), other.clone()];
        let dropped = Arc::new(Mutex::new(false));
        let observed = Arc::new(Mutex::new(Vec::new()));
        {
            let (dir, dropped, observed) = (h.requests.clone(), dropped.clone(), observed.clone());
            e.set_observer(Box::new(move |s: &UpdateStatus| {
                observed.lock().unwrap().push(s.state.clone());
                if let UpdateState::InProgress {
                    step: Step::Pulled, ..
                } = s.state
                {
                    let mut d = dropped.lock().unwrap();
                    if !*d {
                        *d = true;
                        for (i, r) in late.iter().enumerate() {
                            drop_request(&dir, r, 10 - i as u64);
                        }
                    }
                }
            }));
        }
        drop_request(&h.requests, &first, 60);
        let out = process_requests(&mut e, &h.requests, &no_stop())
            .await
            .unwrap();
        assert!(*dropped.lock().unwrap());
        let summary: Vec<_> = out
            .iter()
            .map(|e| (e.request_id.unwrap(), e.outcome, e.detail.clone()))
            .collect();
        assert_eq!(summary.len(), 4, "{summary:?}");
        assert_eq!(summary[0].0, first.id);
        assert_eq!(summary[0].1, Outcome::Succeeded);
        assert_eq!(
            summary[1],
            (dup.id, Outcome::Rejected, UPDATE_IN_PROGRESS.to_string())
        );
        assert_eq!(
            summary[2],
            (chan.id, Outcome::Rejected, UPDATE_IN_PROGRESS.to_string())
        );
        assert_eq!(summary[3].0, other.id);
        assert_eq!(summary[3].1, Outcome::Succeeded, "{}", summary[3].2);
        assert_eq!(
            e.journal_state().active.as_ref().unwrap().version,
            b.version
        );
        assert!(names(&h.requests).is_empty());
        // Status was published while the install ran.
        assert!(observed.lock().unwrap().iter().any(|s| matches!(
            s,
            UpdateState::InProgress {
                step: Step::Probing,
                ..
            }
        )));
    }
}
