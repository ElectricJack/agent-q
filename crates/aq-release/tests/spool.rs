use aq_release::*;
use chrono::Utc;
use semver::Version;
use uuid::Uuid;

#[test]
fn status_json_roundtrip() {
    let now = Utc::now();
    let s = ReleaseSummary {
        version: Version::new(0, 2, 0),
        source_revision: "abc".into(),
        server_image: "r/s@sha256:00".into(),
        state_format: 1,
    };
    let st = UpdateStatus {
        updater_version: "0.1.0".into(),
        state: UpdateState::InProgress {
            request_id: Uuid::new_v4(),
            target: Some(Version::new(0, 2, 0)),
            step: Step::Probing,
            started_at: now,
        },
        active: Some(s.clone()),
        previous: None,
        available: Some(s),
        history: vec![HistoryEntry {
            request_id: None,
            from: Some(Version::new(0, 1, 0)),
            to: Some(Version::new(0, 2, 0)),
            outcome: Outcome::RolledBack,
            detail: "probe failed".into(),
            started_at: now,
            finished_at: now,
        }],
        updated_at: now,
    };
    let j = serde_json::to_vec(&st).unwrap();
    assert_eq!(serde_json::from_slice::<UpdateStatus>(&j).unwrap(), st);
    let idle = UpdateStatus {
        state: UpdateState::Idle,
        ..st.clone()
    };
    let j = serde_json::to_string(&idle).unwrap();
    assert!(j.contains(r#""state":{"kind":"idle"}"#), "{j}");
    let f = UpdateState::FailedNeedsOperator { detail: "x".into() };
    let j = serde_json::to_vec(&f).unwrap();
    assert_eq!(serde_json::from_slice::<UpdateState>(&j).unwrap(), f);
    // A channel request before its target is known: `target` is omitted.
    let unknown = UpdateState::InProgress {
        request_id: Uuid::new_v4(),
        target: None,
        step: Step::Requested,
        started_at: now,
    };
    let j = serde_json::to_string(&unknown).unwrap();
    assert!(!j.contains("target"), "{j}");
    assert_eq!(serde_json::from_str::<UpdateState>(&j).unwrap(), unknown);
}

#[test]
fn step_order_is_declared_order() {
    assert!(
        Step::Requested < Step::Validated
            && Step::Probing < Step::Active
            && Step::Active < Step::RollingBack
    );
}

#[test]
fn request_rejects_unknown_fields() {
    let ok = r#"{"id":"67e55044-10b1-426f-9247-bb680e5fe0c8","version":"0.2.0","requested_by":"owner","requested_at":"2026-10-30T12:00:00Z"}"#;
    let r: UpdateRequest = serde_json::from_str(ok).unwrap();
    assert_eq!(r.version, Some(Version::new(0, 2, 0)));
    let none = ok.replace(r#""version":"0.2.0","#, "");
    assert_eq!(
        serde_json::from_str::<UpdateRequest>(&none)
            .unwrap()
            .version,
        None
    );
    let bad = ok.replace("\"requested_by\"", "\"image\":\"evil\",\"requested_by\"");
    assert!(serde_json::from_str::<UpdateRequest>(&bad).is_err());
}

#[test]
fn readiness_report_roundtrip() {
    let r = ReadinessReport {
        version: Version::new(0, 2, 0),
        source_revision: "abc".into(),
        state_format: 1,
        protocol_min: 1,
        protocol_max: 1,
        database: "ok".into(),
        auth_mode: "password".into(),
    };
    let j = serde_json::to_vec(&r).unwrap();
    assert_eq!(serde_json::from_slice::<ReadinessReport>(&j).unwrap(), r);
    let mut v: serde_json::Value = serde_json::from_slice(&j).unwrap();
    v["extra"] = 1.into();
    assert!(serde_json::from_value::<ReadinessReport>(v).is_err());
}

#[test]
fn status_tolerates_unknown_fields_and_variants() {
    let j = r#"{"updater_version":"9.9.9","future":true,
"state":{"kind":"in_progress","request_id":"67e55044-10b1-426f-9247-bb680e5fe0c8","target":"0.2.0","step":"warming_up","started_at":"2026-10-30T12:00:00Z","extra":1},
"active":{"version":"0.1.0","source_revision":"a","server_image":"i","state_format":1,"new":2},
"previous":null,"available":null,
"history":[{"request_id":null,"from":null,"to":null,"outcome":"vaporized","detail":"","started_at":"2026-10-30T12:00:00Z","finished_at":"2026-10-30T12:00:00Z","more":1}],
"updated_at":"2026-10-30T12:00:00Z"}"#;
    let st: UpdateStatus = serde_json::from_str(j).unwrap();
    assert!(matches!(
        st.state,
        UpdateState::InProgress {
            step: Step::Unknown,
            ..
        }
    ));
    assert_eq!(st.history[0].outcome, Outcome::Unknown);
    let j2 = j.replace(r#""kind":"in_progress""#, r#""kind":"teleporting""#);
    let st: UpdateStatus = serde_json::from_str(&j2).unwrap();
    assert_eq!(st.state, UpdateState::Unknown);
}
