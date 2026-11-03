//! Startup policy structure/provenance only: no actual runtime is qualified here.
use aq_admission::{transport::*, *};
use aq_server::profiled_execution::config::*;
use serde_json::json;
use uuid::Uuid;
fn artifact(id: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(id.as_bytes()),
    }
}
fn config() -> ProfiledConfig {
    ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 1,
        qualified_runtimes: vec![QualifiedRuntime {
            runtime: artifact("synthetic-runtime"),
            qualification: artifact("synthetic-qualification"),
            capabilities: vec![],
            features: vec![Feature::ArtifactTransferV1, Feature::DurableHostJournalV1],
            material_delivery: None,
        }],
        projects: vec![ProjectPolicy {
            project_id: Uuid::from_u128(1),
            enabled: true,
            priority: 0,
            source: SourceSelection::Immutable {
                object: ObjectId {
                    format: ObjectFormat::Sha1,
                    hex: "a".repeat(40),
                },
            },
            retention_handle: Uuid::from_u128(2),
            replacement_budget: None,
            placement: None,
        }],
    }
}
#[test]
fn policy_artifact_binds_global_qualification_and_exact_source_without_user_actor() {
    let cfg = config();
    cfg.validate().unwrap();
    let record = cfg.project_policy_record(Uuid::from_u128(1)).unwrap();
    assert_eq!(
        record.artifact().unwrap(),
        cfg.project_policy_ref(Uuid::from_u128(1)).unwrap()
    );
    let old = record.artifact().unwrap();
    let mut changed = cfg.clone();
    changed.qualified_runtimes[0].qualification = artifact("different-qualified-code");
    assert_ne!(old, changed.project_policy_ref(Uuid::from_u128(1)).unwrap());
    let mut changed = cfg.clone();
    changed.projects[0].source = SourceSelection::Immutable {
        object: ObjectId {
            format: ObjectFormat::Sha1,
            hex: "b".repeat(40),
        },
    };
    assert_ne!(old, changed.project_policy_ref(Uuid::from_u128(1)).unwrap());
    assert!(cfg.project_policy_record(Uuid::from_u128(3)).is_err());
    let mut wrong = serde_json::to_value(&cfg).unwrap();
    wrong["actor_id"] = json!(Uuid::from_u128(4));
    assert!(serde_json::from_value::<ProfiledConfig>(wrong).is_err());
}
#[test]
fn configuration_and_runtime_claims_refuse_duplicates_unsafe_refs_and_unqualified_artifacts() {
    let cfg = config();
    let mut duplicate = cfg.clone();
    duplicate.projects.push(duplicate.projects[0].clone());
    assert!(duplicate.validate().is_err());
    let mut bad = cfg.clone();
    bad.projects[0].source = SourceSelection::NamedRef {
        reference: Label::new("HEAD").unwrap(),
    };
    assert!(bad.validate().is_err());
    let mut request = Register {
        schema_version: 1,
        operation_id: Uuid::from_u128(5),
        identity: WorkerIdentity {
            node_id: Uuid::from_u128(6),
            boot_id: Uuid::from_u128(7),
        },
        runtime: cfg.qualified_runtimes[0].runtime.clone(),
        capabilities: vec![],
        features: cfg.qualified_runtimes[0].features.clone(),
    };
    cfg.qualified(&request).unwrap();
    request.runtime = artifact("arbitrary-worker-claim");
    assert!(cfg.qualified(&request).is_err());
    request.runtime = cfg.qualified_runtimes[0].runtime.clone();
    request.features.push(Feature::ServerMaterialV1);
    assert!(cfg.qualified(&request).is_err());
}

#[test]
fn project_replacement_budget_is_optional_bounded_and_bound_into_the_configuration_digest() {
    use aq_server::profiled_execution::replacement::{
        Limits, MAX_ATTEMPTS, MAX_CONFIGURED_WINDOW_MS, MAX_LINEAGE_ELAPSED_MS, MAX_SANDBOX_MS,
    };
    let cfg = config();
    // Omitted: server defaults, and the historical encoding and digest.
    assert_eq!(cfg.projects[0].replacement_limits(), Limits::default());
    let encoded = serde_json::to_value(&cfg).unwrap();
    assert!(encoded["projects"][0].get("replacement_budget").is_none());
    let digest = cfg.configuration_digest().unwrap();
    let historical = json!({
        "revision": cfg.revision,
        "qualified_runtimes": cfg.qualified_runtimes,
        "projects": [{
            "project_id": cfg.projects[0].project_id,
            "enabled": true,
            "priority": 0,
            "source": cfg.projects[0].source,
            "retention_handle": cfg.projects[0].retention_handle,
        }],
    });
    assert_eq!(encoded, historical);
    assert_eq!(digest, aq_selection::digest(&historical).unwrap());
    let restored: ProfiledConfig = serde_json::from_value(historical).unwrap();
    assert_eq!(restored.configuration_digest().unwrap(), digest);
    assert_eq!(restored.projects[0].replacement_limits(), Limits::default());
    assert_eq!(
        serde_json::from_value::<Limits>(json!({})).unwrap(),
        Limits::default()
    );

    // A partial budget overrides only the named field.
    let mut raw = encoded.clone();
    raw["projects"][0]["replacement_budget"] = json!({"max_attempts": 1});
    let tight: ProfiledConfig = serde_json::from_value(raw.clone()).unwrap();
    tight.validate().unwrap();
    assert_eq!(
        tight.projects[0].replacement_limits(),
        Limits {
            max_attempts: 1,
            max_lineage_elapsed_ms: MAX_LINEAGE_ELAPSED_MS,
            max_sandbox_ms: MAX_SANDBOX_MS,
        }
    );
    assert_ne!(digest, tight.configuration_digest().unwrap());
    let roundtrip: ProfiledConfig =
        serde_json::from_value(serde_json::to_value(&tight).unwrap()).unwrap();
    assert_eq!(roundtrip.projects[0].replacement_limits().max_attempts, 1);

    raw["projects"][0]["replacement_budget"] =
        json!({"max_attempts": 5, "max_lineage_elapsed_ms": 3_600_000, "max_sandbox_ms": 600_000});
    let full: ProfiledConfig = serde_json::from_value(raw.clone()).unwrap();
    full.validate().unwrap();
    assert_eq!(
        full.projects[0].replacement_limits(),
        Limits {
            max_attempts: 5,
            max_lineage_elapsed_ms: 3_600_000,
            max_sandbox_ms: 600_000,
        }
    );
    assert_ne!(MAX_ATTEMPTS, 5);

    for valid in [
        json!({"max_attempts": 1, "max_lineage_elapsed_ms": 1, "max_sandbox_ms": 1}),
        json!({"max_attempts": 32, "max_lineage_elapsed_ms": MAX_CONFIGURED_WINDOW_MS,
               "max_sandbox_ms": MAX_CONFIGURED_WINDOW_MS}),
    ] {
        raw["projects"][0]["replacement_budget"] = valid;
        serde_json::from_value::<ProfiledConfig>(raw.clone())
            .unwrap()
            .validate()
            .unwrap();
    }

    // Zero or unbounded budgets and unknown fields are refused.
    for bad in [
        json!({"max_attempts": 0}),
        json!({"max_attempts": 33}),
        json!({"max_lineage_elapsed_ms": 0}),
        json!({"max_sandbox_ms": 0}),
        json!({"max_sandbox_ms": MAX_CONFIGURED_WINDOW_MS + 1}),
        json!({"max_lineage_elapsed_ms": MAX_CONFIGURED_WINDOW_MS + 1}),
    ] {
        raw["projects"][0]["replacement_budget"] = bad.clone();
        let cfg: ProfiledConfig = serde_json::from_value(raw.clone()).unwrap();
        assert!(cfg.validate().is_err(), "{bad}");
    }
    raw["projects"][0]["replacement_budget"] = json!({"max_retries": 2});
    assert!(serde_json::from_value::<ProfiledConfig>(raw).is_err());
}
