use aq_release::{HistoryEntry, Outcome, Step};
use aq_updater::journal::{Journal, JournalError, JournalState, lock};
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

#[test]
fn first_open_is_empty_and_creates_dir() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("state");
    let j = Journal::open(&dir).unwrap();
    assert!(j.state().active.is_none() && j.state().current.is_none());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o750
    );
}

#[test]
fn record_roundtrips() {
    let t = tempfile::tempdir().unwrap();
    let mut j = Journal::open(t.path()).unwrap();
    let s = JournalState {
        current: Some(in_flight()),
        ..Default::default()
    };
    j.record(s).unwrap();
    let j2 = Journal::open(t.path()).unwrap();
    assert_eq!(j2.state().current.as_ref().unwrap().step, Step::Pulled);
}

#[test]
fn record_is_atomic() {
    let t = tempfile::tempdir().unwrap();
    let mut j = Journal::open(t.path()).unwrap();
    let s = JournalState {
        current: Some(in_flight()),
        ..Default::default()
    };
    j.record(s).unwrap();
    // Crash mid-write: garbage temp file left behind.
    std::fs::write(t.path().join("journal.json.tmp"), b"{\"journal_for").unwrap();
    let j2 = Journal::open(t.path()).unwrap();
    assert_eq!(j2.state().current.as_ref().unwrap().step, Step::Pulled);
    assert!(!t.path().join("journal.json.tmp").exists());
}

#[test]
fn corrupt_journal_reported_not_guessed() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("journal.json"),
        b"{\"journal_format\": 1, \"cur",
    )
    .unwrap();
    match Journal::open(t.path()) {
        Err(JournalError::Corrupt { path, raw }) => {
            assert!(path.ends_with("journal.json"));
            assert!(raw.contains("journal_format"));
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
    // The file is left untouched.
    assert!(
        std::fs::read(t.path().join("journal.json"))
            .unwrap()
            .starts_with(b"{\"journal_format")
    );
}

#[test]
fn corrupt_raw_is_capped() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("journal.json"), vec![b'x'; 200_000]).unwrap();
    match Journal::open(t.path()) {
        Err(JournalError::Corrupt { raw, .. }) => assert_eq!(raw.len(), 64 * 1024),
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn unknown_format_refused() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("journal.json"),
        br#"{"journal_format": 2, "active": null, "previous": null, "current": null}"#,
    )
    .unwrap();
    let e = Journal::open(t.path()).unwrap_err();
    assert!(
        matches!(e, JournalError::UnsupportedFormat { found: 2, .. }),
        "{e:?}"
    );
}

#[test]
fn history_appends_and_reads_newest_first() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(t.path()).unwrap();
    assert!(j.recent_history(5).is_empty());
    for n in 1..=3 {
        j.append_history(entry(n)).unwrap();
    }
    let r = j.recent_history(2);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].detail, "e3");
    assert_eq!(r[1].detail, "e2");
}

#[test]
fn torn_history_line_skipped() {
    let t = tempfile::tempdir().unwrap();
    let j = Journal::open(t.path()).unwrap();
    j.append_history(entry(1)).unwrap();
    j.append_history(entry(2)).unwrap();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(t.path().join("history.jsonl"))
        .unwrap();
    f.write_all(b"{\"request_id\":nu").unwrap();
    let r = j.recent_history(10);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].detail, "e2");
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
