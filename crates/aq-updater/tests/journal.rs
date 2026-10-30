use aq_release::{HistoryEntry, Outcome, Step};
use aq_updater::journal::{Journal, JournalError, JournalState, lock, read_history};
use chrono::Utc;

fn entry(n: u64) -> HistoryEntry {
    HistoryEntry {
        request_id: None,
        from: None,
        to: Some(semver::Version::new(1, 0, n)),
        outcome: Outcome::Succeeded,
        detail: format!("e{n}"),
        started_at: Utc::now(),
        finished_at: Utc::now(),
    }
}

fn in_flight() -> aq_updater::journal::InFlight {
    // Built through serde so the test does not need a full manifest builder.
    serde_json::from_value(serde_json::json!({
        "request_id": uuid::Uuid::nil(),
        "target": manifest_json(),
        "step": "pulled",
        "backup": "/b/x.dump",
        "needs_restore_on_rollback": true,
        "started_at": Utc::now(),
        "rollback_attempted": false
    }))
    .unwrap()
}

fn manifest_json() -> serde_json::Value {
    let d = "a".repeat(64);
    serde_json::json!({
        "manifest_version": 1,
        "version": "1.2.3",
        "source_revision": "abc",
        "images": {
            "server": format!("ghcr.io/x/server@sha256:{d}"),
            "worker": format!("ghcr.io/x/worker@sha256:{d}")
        },
        "protocol": {"min": 1, "max": 1},
        "state_format": 1,
        "readable_state_formats": [1],
        "upgrade_from": ">=0.1.0",
        "created_at": Utc::now()
    })
}

fn write_journal(dir: &std::path::Path, v: serde_json::Value) {
    std::fs::write(dir.join("journal.json"), serde_json::to_vec(&v).unwrap()).unwrap();
}

fn on_disk() -> serde_json::Value {
    serde_json::json!({
        "journal_format": 1,
        "active": null,
        "previous": null,
        "current": null
    })
}

fn assert_corrupt(dir: &std::path::Path) {
    match Journal::read(dir) {
        Err(JournalError::Corrupt { path, .. }) => assert!(path.ends_with("journal.json")),
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn first_open_is_empty_and_creates_dir() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("state");
    let j = Journal::open(lock(&dir).unwrap()).unwrap();
    assert!(j.state().active.is_none() && j.state().current.is_none());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o750
    );
}

#[test]
fn file_modes_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let mut j = Journal::open(lock(t.path()).unwrap()).unwrap();
    j.record(JournalState::default()).unwrap();
    j.append_history(entry(1)).unwrap();
    for f in ["journal.json", "history.jsonl"] {
        let m = std::fs::metadata(t.path().join(f))
            .unwrap()
            .permissions()
            .mode();
        // The umask may tighten but never loosen: no group-write, no other access.
        assert_eq!(m & 0o037, 0, "{f} mode {m:o}");
    }
}

#[test]
fn record_roundtrips() {
    let t = tempfile::tempdir().unwrap();
    let mut j = Journal::open(lock(t.path()).unwrap()).unwrap();
    let s = JournalState {
        current: Some(in_flight()),
        ..Default::default()
    };
    j.record(s).unwrap();
    let read = Journal::read(t.path()).unwrap();
    assert_eq!(read.current.as_ref().unwrap().step, Step::Pulled);
}

#[test]
fn record_is_atomic() {
    let t = tempfile::tempdir().unwrap();
    let mut j = Journal::open(lock(t.path()).unwrap()).unwrap();
    let s = JournalState {
        current: Some(in_flight()),
        ..Default::default()
    };
    j.record(s).unwrap();
    // Crash mid-write: garbage temp file left behind.
    std::fs::write(t.path().join("journal.json.tmp"), b"{\"journal_for").unwrap();
    drop(j);
    let j2 = Journal::open(lock(t.path()).unwrap()).unwrap();
    assert_eq!(j2.state().current.as_ref().unwrap().step, Step::Pulled);
    assert!(!t.path().join("journal.json.tmp").exists());
}

#[test]
fn read_never_touches_tmp_or_creates_dir() {
    let t = tempfile::tempdir().unwrap();
    let tmp = t.path().join("journal.json.tmp");
    std::fs::write(&tmp, b"in-flight write").unwrap();
    let s = Journal::read(t.path()).unwrap();
    assert!(s.current.is_none());
    assert_eq!(std::fs::read(&tmp).unwrap(), b"in-flight write");
    let missing = t.path().join("nope");
    assert!(Journal::read(&missing).unwrap().active.is_none());
    assert!(!missing.exists());
    assert!(read_history(&missing, 5).unwrap().is_empty());
    assert!(!missing.exists());
}

#[test]
fn corrupt_journal_reported_not_guessed() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("journal.json"),
        b"{\"journal_format\": 1, \"cur",
    )
    .unwrap();
    match Journal::read(t.path()) {
        Err(JournalError::Corrupt { path, raw }) => {
            assert!(path.ends_with("journal.json"));
            assert!(raw.contains("journal_format"));
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
    // The writer path reports it too, and leaves the file untouched.
    assert!(matches!(
        Journal::open(lock(t.path()).unwrap()),
        Err(JournalError::Corrupt { .. })
    ));
    assert!(
        std::fs::read(t.path().join("journal.json"))
            .unwrap()
            .starts_with(b"{\"journal_format")
    );
}

#[test]
fn corrupt_message_does_not_suggest_removal() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("journal.json"), b"junk").unwrap();
    let e = Journal::read(t.path()).unwrap_err().to_string();
    assert!(!e.contains("or remove"), "{e}");
    assert!(e.contains("lose"), "{e}");
}

#[test]
fn corrupt_raw_is_capped() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("journal.json"), vec![b'x'; 200_000]).unwrap();
    match Journal::read(t.path()) {
        Err(JournalError::Corrupt { raw, .. }) => assert_eq!(raw.len(), 64 * 1024),
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn zero_length_journal_is_corrupt() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("journal.json"), b"").unwrap();
    assert_corrupt(t.path());
}

#[test]
fn missing_format_is_corrupt() {
    let t = tempfile::tempdir().unwrap();
    write_journal(
        t.path(),
        serde_json::json!({"active": null, "previous": null, "current": null}),
    );
    assert_corrupt(t.path());
}

#[test]
fn unknown_field_is_corrupt() {
    let t = tempfile::tempdir().unwrap();
    let mut v = on_disk();
    v["surprise"] = serde_json::json!(1);
    write_journal(t.path(), v);
    assert_corrupt(t.path());
}

#[test]
fn bogus_step_is_corrupt() {
    let t = tempfile::tempdir().unwrap();
    let mut v = on_disk();
    v["current"] = serde_json::to_value(in_flight()).unwrap();
    v["current"]["step"] = serde_json::json!("bogus");
    write_journal(t.path(), v);
    assert_corrupt(t.path());
}

#[test]
fn invalid_manifest_in_journal_is_corrupt() {
    let t = tempfile::tempdir().unwrap();
    let mut m = manifest_json();
    m["images"]["server"] = serde_json::json!("ghcr.io/x:latest");
    let mut v = on_disk();
    v["previous"] = m;
    write_journal(t.path(), v);
    assert_corrupt(t.path());
}

#[test]
fn unknown_format_refused() {
    let t = tempfile::tempdir().unwrap();
    let mut v = on_disk();
    v["journal_format"] = serde_json::json!(2);
    write_journal(t.path(), v);
    let e = Journal::read(t.path()).unwrap_err();
    assert!(
        matches!(e, JournalError::UnsupportedFormat { found: 2, .. }),
        "{e:?}"
    );
}

#[test]
fn history_appends_and_reads_newest_first() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(lock(t.path()).unwrap()).unwrap();
    assert!(j.recent_history(5).unwrap().is_empty());
    for n in 1..=3 {
        j.append_history(entry(n)).unwrap();
    }
    let r = j.recent_history(2).unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].detail, "e3");
    assert_eq!(r[1].detail, "e2");
}

#[test]
fn torn_history_line_skipped() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(lock(t.path()).unwrap()).unwrap();
    j.append_history(entry(1)).unwrap();
    j.append_history(entry(2)).unwrap();
    tear(t.path());
    let r = j.recent_history(10).unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].detail, "e2");
}

fn tear(dir: &std::path::Path) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("history.jsonl"))
        .unwrap();
    f.write_all(b"{\"request_id\":nu").unwrap();
}

#[test]
fn append_after_torn_line_is_readable() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(lock(t.path()).unwrap()).unwrap();
    j.append_history(entry(1)).unwrap();
    tear(t.path());
    j.append_history(entry(2)).unwrap();
    let r = j.recent_history(10).unwrap();
    assert_eq!(
        r.iter().map(|e| e.detail.as_str()).collect::<Vec<_>>(),
        ["e2", "e1"]
    );
}

#[test]
fn history_read_error_is_returned() {
    let t = tempfile::tempdir().unwrap();
    // A directory where the file should be: read fails with something other than NotFound.
    std::fs::create_dir(t.path().join("history.jsonl")).unwrap();
    assert!(read_history(t.path(), 5).is_err());
}

#[test]
fn second_lock_fails_fast() {
    let t = tempfile::tempdir().unwrap();
    let first = lock(t.path()).unwrap();
    let e = lock(t.path()).expect_err("second lock must fail");
    assert!(
        e.to_string().contains("another aq-updater is running"),
        "{e}"
    );
    assert!(e.to_string().contains("lock"), "{e}");
    drop(first);
    lock(t.path()).expect("lock is released on drop");
}

#[test]
fn journal_is_send_static() {
    fn assert_send<T: Send + 'static>() {}
    assert_send::<Journal>();
}

#[test]
fn dropping_journal_releases_lock() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(lock(t.path()).unwrap()).unwrap();
    assert_eq!(j.state_dir(), t.path());
    assert!(lock(t.path()).is_err());
    drop(j);
    lock(t.path()).expect("lock released with the journal");
}
