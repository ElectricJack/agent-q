//! Shared explicitly synthetic qualification, actual native owner setup.
use crate::common;
use aq_admission::{ArtifactKind, ArtifactRef, Digest, Text, scoped_presence::*};
use aq_server::profiled_execution::{definitions::Definition, policies};
use uuid::Uuid;
pub fn definition(name: &str, body: String) -> Definition {
    Definition::new(
        ArtifactRef {
            id: Text::new(name).unwrap(),
            version: Text::new("1").unwrap(),
            digest: Digest::of(body.as_bytes()),
        },
        ArtifactKind::Definition,
        body,
    )
    .unwrap()
}
pub struct Case {
    pub c: common::native_offer::Case,
    pub request: InventoryV2,
}
pub async fn setup() -> Case {
    let mut c = common::native_offer::setup().await;

    let mut config = (*c.env.env.state.config).clone();
    let mut profiled = (**config.profiled_execution.as_ref().unwrap()).clone();
    let runtime = &profiled.qualified_runtimes[0];
    let adapter = definition(
        "aq.presence-adapter/test",
        "Synthetic actual private-read adapter definition".into(),
    );
    let evidence = definition(
        "aq.presence-evidence/test",
        "Synthetic tests only; no physical producer qualification".into(),
    );
    let q = Qualification {
        schema_version: 1,
        protocol: Protocol::ScopedMaterialPresenceV1,
        runtime: runtime.runtime.clone(),
        runtime_qualification: runtime.qualification.clone(),
        adapter: adapter.artifact().clone(),
        adapter_qualification: evidence.artifact().clone(),
        inventory_semantics: InventorySemantics::ProvisionableOnlyAndScopedPresenceV1,
        observation_protocol: ObservationProtocol::GrantIssuanceBoundedBoottimeV1,
        maximum_age_ms: 30_000,
        maximum_ttl_ms: 30_000,
    };
    let body = serde_json::to_string(&q).unwrap();
    let qdef = definition("aq.scoped-material-presence-qualification/test", body);
    let request = InventoryV2 {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: c.request.identity.clone(),
        runtime: q.runtime.clone(),
        runtime_qualification: q.runtime_qualification.clone(),
        negotiation: Negotiation {
            protocol: Protocol::ScopedMaterialPresenceV1,
            qualification: qdef.artifact().clone(),
        },
        sequence: 2,
        ttl_ms: 30_000,
        qualifications: vec![c.env.input.node.harnesses[0].qualification.clone()],
        provisionable_only: vec![CredentialReference {
            profile: c.env.input.credentials[0].profile.clone(),
            revision: c.env.input.credentials[0].revision,
        }],
    };
    profiled.definition_bodies.extend([adapter, evidence, qdef]);
    profiled.revision += 1;
    config.profiled_execution = Some(std::sync::Arc::new(profiled));
    c.env.env.state.config = std::sync::Arc::new(config);
    policies::install(&c.env.env.state).await.unwrap();
    Case { c, request }
}
