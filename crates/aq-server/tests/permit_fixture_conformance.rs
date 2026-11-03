//! Shared owner vectors exercise the exact delegated contract consumed by the server.
use aq_admission_sandbox::sandbox::{self, Fixture, FixtureCase, SandboxDocument};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

const FIXTURES: &str =
    include_str!("../../../vendor/aq-admission-0.2.0/artifacts/sandbox-v1.fixtures.json");

#[test]
fn permit_fixture_conformance() {
    let fixtures: Vec<Fixture> = serde_json::from_str(FIXTURES).unwrap();
    let mut cases = BTreeMap::new();
    let mut outcomes = BTreeMap::new();
    for fixture in &fixtures {
        assert_eq!(
            sandbox::validate_fixture(fixture),
            fixture.expected,
            "owner vector {}",
            fixture.name
        );
        let kind = match fixture.case {
            FixtureCase::Permit { .. } => "permit",
            FixtureCase::Readoption { .. } => "readoption",
            FixtureCase::ReadoptionReplay { .. } => "replay",
            FixtureCase::Compatibility { .. } => "compatibility",
            FixtureCase::Registration { .. } => "registration",
            FixtureCase::PhaseHandoff { .. } => "handoff",
            FixtureCase::RunnerRenewal { .. } => "renewal",
        };
        *cases.entry(kind).or_insert(0) += 1;
        *outcomes.entry(fixture.expected.as_str()).or_insert(0) += 1;
    }
    // Literal release inventory prevents an empty or partially copied corpus passing.
    assert_eq!(
        cases,
        BTreeMap::from([
            ("permit", 15),
            ("readoption", 5),
            ("replay", 4),
            ("compatibility", 3),
            ("registration", 3),
            ("handoff", 4),
            ("renewal", 10),
        ])
    );
    assert_eq!(
        outcomes,
        BTreeMap::from([
            ("accepted", 12),
            ("binding_mismatch", 9),
            ("deadline_mismatch", 6),
            ("expired", 4),
            ("invalid_shape", 3),
            ("replay_conflict", 3),
            ("scope_denied", 5),
            ("unsupported_required_capability", 2),
        ])
    );
    let published_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../vendor/aq-admission-0.2.0/artifacts/sandbox-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(sandbox::schema(), published_schema);
}

#[test]
fn permit_fixture_conformance_source_pin() {
    #[derive(Deserialize)]
    struct Pin {
        source_revision: String,
        archive: String,
        archive_sha256: String,
        directory: String,
        files: BTreeMap<String, String>,
    }
    let pin: Pin =
        serde_json::from_str(include_str!("../../../vendor/admission-contract.json")).unwrap();
    assert_eq!(pin.source_revision.len(), 40);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let archive = fs::read(root.join(pin.archive)).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(archive)), pin.archive_sha256);
    assert_eq!(pin.files.len(), 119);
    for (path, expected) in pin.files {
        let actual = fs::read(root.join(&pin.directory).join(&path)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(actual)), expected, "{path}");
    }
}

#[test]
fn permit_fixture_conformance_frozen_transport() {
    let legacy: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../aq-admission/artifacts/transport.fixtures.json"
    ))
    .unwrap();
    assert!(!legacy.is_empty());
    for value in legacy {
        let bytes = serde_json::to_vec(&value).unwrap();
        let frozen = aq_admission::transport::decode_transport(&bytes).unwrap();
        let next = aq_admission_sandbox::transport::decode_transport(&bytes).unwrap();
        assert_eq!(serde_json::to_value(frozen).unwrap(), value);
        assert_eq!(serde_json::to_value(next).unwrap(), value);
    }
    let fixtures: Vec<Fixture> = serde_json::from_str(FIXTURES).unwrap();
    let registration = fixtures
        .iter()
        .find(|f| f.name == "valid_registration")
        .unwrap();
    let FixtureCase::Registration { registration } = &registration.case else {
        panic!("valid_registration must exercise the registration envelope");
    };
    let bytes = serde_json::to_vec(&SandboxDocument::Register(registration.clone())).unwrap();
    sandbox::decode_document(&bytes).unwrap();
    // Frozen endpoints explicitly reject the new envelope during negotiation.
    assert!(aq_admission::transport::decode_transport(&bytes).is_err());
}
