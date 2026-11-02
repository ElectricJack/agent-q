//! Fixture-only owner assessment of a live finite local Responses peer.
//! Reads real local facts; never rewrites liveness, inventory, selection or grants.
#![allow(dead_code)] // Composed fixture includes this helper separately.
use super::{
    codex_runtime_recipe::{self as recipe, Prepared, ProgramPin},
    native_runner_transport::PrivateState,
};
use anyhow::{Context, Result, ensure};
use aq_admission::{
    Digest, preparation_refresh_v1 as phase,
    scoped_presence::{InventoryReceipt, InventoryV2},
    transport::{TransportDocument, WorkerIdentity},
};
use aq_selection::{
    AvailabilityObservation, CapabilitySnapshot, ExecutionTarget, ObservationScope,
};
use aq_server::{AppState, profiled_execution::registration::NativeRegistration};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use sqlx::Row;
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::Read,
    net::TcpListener,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
use tokio::task::JoinHandle;
use tower::ServiceExt;
use uuid::Uuid;

pub struct LocalImage {
    pub guest: ProgramPin,
    pub outer: PathBuf,
}
pub struct LocalEvidence {
    /// Exactly the five actual images from the retained runtime/environment definitions.
    pub images: Vec<LocalImage>,
    pub synthetic_key_file: PathBuf,
    pub synthetic_key_digest: Digest,
    pub synthetic_key_bytes: u64,
}
pub struct PeerWitness<'a, T> {
    pub listener: &'a TcpListener,
    pub server: &'a JoinHandle<T>,
    pub origin: &'a str,
}
pub enum Stage<'a> {
    BeforePoll,
    BeforePrepare(&'a phase::PrepareRequest),
}
#[derive(Default)]
pub struct Assessor {
    sequence: u64,
    last: Option<(Uuid, Value)>,
    subject: Option<Value>,
}

fn canonical(value: &Value) -> Result<Vec<u8>> {
    let mut value = value.clone();
    value.sort_all_objects();
    Ok(serde_json::to_vec(&value)?)
}
fn current_ms(state: &AppState) -> Result<u64> {
    let now = u64::try_from((state.clock)().timestamp_millis())?;
    let real = u64::try_from(chrono::Utc::now().timestamp_millis())?;
    ensure!(
        now.abs_diff(real) <= 1000,
        "assessment requires actual wall clock"
    );
    Ok(now)
}
fn file(path: &Path, bytes: u64, private: bool) -> Result<std::fs::File> {
    ensure!(
        path.is_absolute() && bytes > 0 && bytes <= 512 * 1024 * 1024,
        "bounded actual file"
    );
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK)
        .open(path)?;
    let m = file.metadata()?;
    ensure!(
        m.is_file() && m.len() == bytes,
        "actual regular file length"
    );
    if private {
        ensure!(
            bytes <= 65536
                && m.mode() & 0o7777 == 0o600
                && m.nlink() == 1
                && m.uid() == unsafe { nix::libc::geteuid() },
            "owned private synthetic material"
        );
    }
    Ok(file)
}
fn rehash(path: &Path, bytes: u64, digest: &Digest, private: bool) -> Result<()> {
    let mut file = file(path, bytes, private)?;
    let before = file.metadata()?;
    let mut hash = Sha256::new();
    let mut chunk = zeroize::Zeroizing::new([0u8; 65536]);
    let mut count = 0u64;
    loop {
        let n = file.read(&mut *chunk)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        ensure!(count <= bytes, "actual file grew");
        hash.update(&chunk[..n]);
    }
    let after = file.metadata()?;
    ensure!(
        count == bytes
            && format!("{:x}", hash.finalize()) == digest.as_str()
            && before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec(),
        "actual file changed or pin mismatch"
    );
    Ok(())
}
fn peer<T>(witness: &PeerWitness<'_, T>) -> Result<()> {
    ensure!(!witness.server.is_finished(), "finite local server exited");
    let addr = witness.listener.local_addr()?;
    ensure!(
        addr.ip() == std::net::Ipv4Addr::LOCALHOST
            && addr.port() > 0
            && witness.origin == format!("http://{addr}/"),
        "exact numeric finite peer origin"
    );
    let mut listening = 0i32;
    let mut size = std::mem::size_of_val(&listening) as nix::libc::socklen_t;
    ensure!(
        unsafe {
            nix::libc::getsockopt(
                witness.listener.as_raw_fd(),
                nix::libc::SOL_SOCKET,
                nix::libc::SO_ACCEPTCONN,
                (&mut listening as *mut i32).cast(),
                &mut size,
            )
        } == 0
            && listening == 1
            && size as usize == std::mem::size_of_val(&listening),
        "actual finite peer listening witness"
    );
    Ok(())
}
fn images(prepared: &Prepared, evidence: &LocalEvidence) -> Result<()> {
    let body = |pin: &aq_admission::ArtifactRef| -> Result<Value> {
        let definition = prepared
            .profiled
            .definition_bodies
            .iter()
            .find(|d| d.artifact() == pin)
            .context("retained public image definition")?;
        ensure!(
            Digest::of(definition.body().as_bytes()) == pin.digest,
            "public definition bytes"
        );
        Ok(serde_json::from_str(definition.body())?)
    };
    let runtime = body(&prepared.runtime)?;
    let environment = body(&prepared.environment)?;
    let expected = [
        &runtime["worker"],
        &environment["images"]["codex"],
        &environment["images"]["host"],
        &environment["images"]["tmux"],
        &environment["images"]["client"],
    ];
    ensure!(
        evidence.images.len() == 5,
        "all five actual images required"
    );
    let mut seen = BTreeSet::new();
    for image in &evidence.images {
        let guest = serde_json::to_value(&image.guest)?;
        ensure!(
            expected.contains(&&guest) && seen.insert(image.guest.path.clone()),
            "exact unique retained guest image"
        );
        rehash(
            &image.outer,
            image.guest.bytes,
            &image.guest.artifact.digest,
            false,
        )?;
    }
    Ok(())
}
async fn human(state: &AppState, path: &str, body: Option<Value>) -> Result<Value> {
    let mut request = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("x-aq-dev-email", "owner@localhost");
    if body.is_some() {
        request = request
            .header("content-type", "application/json")
            .header("x-aq-request", "1")
            .header("origin", &state.config.server.public_origin);
    }
    let response = aq_server::build_admin_router(state.clone())
        .oneshot(
            request.body(Body::from(
                body.map(|v| serde_json::to_vec(&v))
                    .transpose()?
                    .unwrap_or_default(),
            ))?,
        )
        .await?;
    ensure!(
        response.status() == StatusCode::OK,
        "protected owner assessment route refused"
    );
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), 1048576).await?,
    )?)
}
fn journal(private: &PrivateState, node: Uuid) -> Result<(Value, WorkerIdentity, Digest)> {
    let bytes = private.read(Path::new("journal/journal.json"), 4194304)?;
    let snapshot: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        snapshot["state"]["format"] == 12
            && snapshot["digest"] == json!(Digest::of(&canonical(&snapshot["state"])?)),
        "actual journal format/digest"
    );
    let identity: WorkerIdentity = serde_json::from_value(snapshot["state"]["identity"].clone())?;
    identity.validate()?;
    ensure!(identity.node_id == node, "actual journal node identity");
    Ok((snapshot["state"].clone(), identity, Digest::of(&bytes)))
}
fn prepared_inputs(
    private: &PrivateState,
    state: &Value,
    identity: &WorkerIdentity,
    request: &phase::PrepareRequest,
    prepared: &Prepared,
    evidence: &LocalEvidence,
    now: u64,
) -> Result<()> {
    let attempt = &state["attempts"][request.original.binding.attempt_id.to_string()];
    let accepted: phase::AcceptedDelivery =
        serde_json::from_value(attempt["refresh_preparation"]["delivery"].clone())?;
    let accept: phase::AcceptRequest =
        serde_json::from_value(attempt["refresh_acceptance"]["request"].clone())?;
    let proposal: phase::Proposal =
        serde_json::from_value(attempt["refresh_acceptance"]["proposal"].clone())?;
    request.validate_for(&accepted.receipt, &accept, &proposal)?;
    ensure!(
        request.original.identity == *identity
            && now < accepted.receipt.lease.expires_at_ms
            && attempt["refresh_prepared"]["request"] == json!(request)
            && attempt["refresh_prepared"]["request_digest"]
                == json!(Digest::of(&canonical(&json!(request))?))
            && state["revision"].as_u64().context("journal revision")?
                > request.original.journal_revision
            && attempt["refresh_execution"].is_null()
            && attempt["native_host"].is_null(),
        "actual retained Prepared request before execution"
    );
    ensure!(
        attempt["source"]["verified"].is_object()
            && attempt["downloads"]
                .as_object()
                .context("actual downloads")?
                .values()
                .all(|d| d["receipt"].is_object()),
        "retained actual source/artifact inputs"
    );
    ensure!(
        request.original.materials.len() == 1 && accepted.receipt.lease.materials.len() == 1,
        "single finite-peer material"
    );
    let material = &request.original.materials[0];
    let lease = &accepted.receipt.lease.materials[0];
    material.validate_against(lease, material.received_at_ms)?;
    ensure!(
        material.material_digest == evidence.synthetic_key_digest
            && lease.material_bytes == evidence.synthetic_key_bytes
            && lease.credential_profile == prepared.credential,
        "actual scoped synthetic material pin"
    );
    let bytes = zeroize::Zeroizing::new(
        private.read(
            &PathBuf::from("materials")
                .join(material.lease_id.to_string())
                .join("material.bin"),
            65536,
        )?,
    );
    ensure!(
        bytes.len() as u64 == lease.material_bytes
            && Digest::of(&bytes) == material.material_digest,
        "actual installed finite-peer material bytes"
    );
    Ok(())
}
impl Assessor {
    /// The caller serializes these two fixture callbacks. No database locks are
    /// held while reading local images or executing the protected owner routes.
    #[allow(clippy::too_many_arguments)] // Explicit borrowed fixture owners, no authority reconstruction.
    pub async fn assess<T>(
        &mut self,
        state: &AppState,
        pool: &sqlx::PgPool,
        node: Uuid,
        prepared: &Prepared,
        private: &PrivateState,
        evidence: &LocalEvidence,
        witness: PeerWitness<'_, T>,
        stage: Stage<'_>,
    ) -> Result<Value> {
        ensure!(
            matches!(
                (&stage, self.sequence),
                (Stage::BeforePoll, 0) | (Stage::BeforePrepare(_), 1)
            ),
            "only the two actual fixture phases may assess"
        );
        let started = current_ms(state)?;
        peer(&witness)?;
        images(prepared, evidence)?;
        rehash(
            &evidence.synthetic_key_file,
            evidence.synthetic_key_bytes,
            &evidence.synthetic_key_digest,
            true,
        )?;
        for (path, expected) in prepared.files()? {
            ensure!(
                private.read(&PathBuf::from("config").join(path), 2097152)? == expected.as_bytes(),
                "actual retained native config bytes"
            );
        }
        let (journal, identity, journal_digest) = journal(private, node)?;
        match stage {
            Stage::BeforePoll => ensure!(
                journal["attempts"]
                    .as_object()
                    .context("journal attempts")?
                    .is_empty(),
                "first actual Poll only"
            ),
            Stage::BeforePrepare(request) => prepared_inputs(
                private,
                &journal,
                &identity,
                request,
                prepared,
                evidence,
                current_ms(state)?,
            )?,
        }
        let (last_seen, revoked): (Option<chrono::DateTime<chrono::Utc>>, bool) =
            sqlx::query_as("SELECT last_seen_at,revoked_at IS NOT NULL FROM nodes WHERE id=$1")
                .bind(node)
                .fetch_one(pool)
                .await?;
        let last = u64::try_from(
            last_seen
                .context("actual awaited heartbeat absent")?
                .timestamp_millis(),
        )?;
        let now = current_ms(state)?;
        ensure!(
            !revoked && now >= last && now - last <= 30000,
            "actual current heartbeat required"
        );
        let execution: Value =
            sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1")
                .bind(node)
                .fetch_one(pool)
                .await?;
        let registration: NativeRegistration = serde_json::from_value(execution["native"].clone())?;
        TransportDocument::Register(registration.request.clone()).validate()?;
        ensure!(
            execution["identity"] == json!(identity)
                && registration.request.identity == identity
                && registration.request.runtime == prepared.runtime
                && registration.qualification == prepared.qualification
                && registration.semantic_digest
                    == aq_server::profiled_execution::registration::semantic_digest(
                        &registration.request
                    )
                    .map_err(|_| anyhow::anyhow!("registration semantic digest"))?,
            "actual exact native registration"
        );
        let config = state
            .config
            .profiled_execution
            .as_ref()
            .context("actual runtime config")?;
        ensure!(
            config
                .qualified(&registration.request)
                .map_err(anyhow::Error::msg)?
                .qualification
                == prepared.qualification,
            "current configured native qualification"
        );
        let registry = state
            .config
            .intelligence
            .as_ref()
            .context("actual intelligence config")?;
        let configured = registry
            .config
            .credentials
            .iter()
            .find(|c| c.credential.profile == prepared.credential)
            .context("exact finite-peer credential")?;
        let [context] = prepared.profiled.context_qualifications.as_slice() else {
            anyhow::bail!("one original finite-peer context subject required");
        };
        context.validate()?;
        let original = &context.subject;
        let subject = serde_json::to_value(original)?;
        ensure!(
            context.artifact()? == prepared.renderer
                && original.runtime == prepared.runtime
                && original.runtime_qualification == prepared.qualification
                && original.adapter == prepared.adapter
                && original.environment == prepared.environment
                && original.harness == prepared.harness.harness
                && original.cli_version == prepared.harness.cli_version
                && original.harness_qualification == prepared.harness.qualification
                && original.credential_profile == prepared.credential
                && original.credential_profile == configured.credential.profile
                && original.credential_revision == configured.credential.revision
                && original.provider == configured.credential.provider
                && original.account_reference == configured.credential.account_reference
                && original.endpoint_reference == configured.credential.endpoint_reference
                && original.model == recipe::MODEL
                && original.native_reasoning == recipe::REASONING
                && original.reasoning_configuration_reference == recipe::REASONING_REFERENCE
                && self.subject.as_ref().is_none_or(|prior| prior == &subject),
            "unchanged original finite-peer qualification subject"
        );
        ensure!(
            configured.allowed_node_ids.contains(&node)
                && configured.secret_file.as_ref() == Some(&evidence.synthetic_key_file)
                && configured.credential.enabled
                && configured.credential.provider == "openai"
                && configured.credential.account_reference == "finite-responses-account"
                && configured.credential.endpoint_reference == "finite-private-loopback-responses"
                && registry
                    .credentials
                    .iter()
                    .any(|c| c.profile == prepared.credential && c.provisionable),
            "actual scoped provisionable material config"
        );
        let snapshot: CapabilitySnapshot = serde_json::from_value(
            sqlx::query_scalar::<_, Value>(
                "SELECT snapshot FROM intelligence_node_snapshots WHERE node_id=$1 AND boot_id=$2",
            )
            .bind(node)
            .bind(identity.boot_id)
            .fetch_one(pool)
            .await?,
        )?;
        ensure!(
            snapshot.target
                == ExecutionTarget::Worker {
                    node_id: node,
                    boot_id: identity.boot_id
                }
                && now >= snapshot.observed_at_ms
                && now < snapshot.expires_at_ms
                && json!(snapshot.harnesses) == json!([prepared.harness])
                && snapshot.credential_bindings.len() == 1
                && snapshot.credential_bindings[0].credential_profile == prepared.credential
                && snapshot.credential_bindings[0].credential_revision
                    == configured.credential.revision
                && !snapshot.credential_bindings[0].available
                && snapshot.credential_bindings[0].preparation.is_some()
                && now >= snapshot.credential_bindings[0].observed_at_ms
                && now < snapshot.credential_bindings[0].expires_at_ms,
            "current real global provisionability inventory, not installed presence"
        );
        let row = sqlx::query("SELECT o.request_bytes,o.receipt_bytes,h.snapshot_digest,h.inventory_revision FROM profiled_inventory_semantic_heads h JOIN profiled_presence_operations o ON o.node_id=h.node_id AND o.operation_id=h.operation_id WHERE h.node_id=$1 AND h.boot_id=$2 AND h.mode='scoped_v1' AND o.kind='inventory_v2'").bind(node).bind(identity.boot_id).fetch_one(pool).await?;
        let inventory: InventoryV2 =
            serde_json::from_slice(&row.get::<Vec<u8>, _>("request_bytes"))?;
        let receipt: InventoryReceipt =
            serde_json::from_slice(&row.get::<Vec<u8>, _>("receipt_bytes"))?;
        receipt.validate_for(&inventory)?;
        ensure!(
            inventory.identity == identity
                && inventory.runtime == prepared.runtime
                && inventory.runtime_qualification == prepared.qualification
                && receipt.inventory.revision == snapshot.revision
                && row.get::<i64, _>("inventory_revision") as u64 == snapshot.revision
                && row.get::<String, _>("snapshot_digest")
                    == Digest::of(&canonical(&json!(snapshot))?).as_str()
                && receipt.inventory.snapshot_digest.as_str()
                    == row.get::<String, _>("snapshot_digest")
                && now >= receipt.recorded_at_ms
                && now < receipt.expires_at_ms,
            "exact actual inventory receipt/head/snapshot"
        );
        let scope = ObservationScope {
            credential_revision: configured.credential.revision,
            provider: configured.credential.provider.clone(),
            credential_profile: prepared.credential.clone(),
            account_reference: configured.credential.account_reference.clone(),
            endpoint_reference: configured.credential.endpoint_reference.clone(),
            model: Some(recipe::MODEL.into()),
            node_id: Some(node),
        };
        let before = human(state, "/api/intelligence/registry", None).await?;
        let revision = before["revision"]
            .as_u64()
            .context("actual registry revision")?;
        let operation = Uuid::new_v4();
        let sequence = self
            .sequence
            .checked_add(1)
            .context("assessment sequence overflow")?;
        peer(&witness)?;
        let checked_at = current_ms(state)?;
        ensure!(
            checked_at
                .checked_sub(started)
                .is_some_and(|elapsed| elapsed < 5000)
                && checked_at < snapshot.expires_at_ms
                && checked_at < receipt.expires_at_ms,
            "local assessment exceeded unchanged freshness age"
        );
        let observed = human(state, "/api/intelligence/observations", Some(json!({"operation_id":operation,"expected_revision":revision,"sequence":sequence,"scope":scope,"state":"available","ttl_ms":30000,"cooldown_ms":0}))).await?;
        let typed: AvailabilityObservation = serde_json::from_value(observed.clone())?;
        typed.validate()?;
        ensure!(
            typed.scope == scope
                && typed.sequence == sequence
                && typed.state == aq_selection::Availability::Available
                && matches!(
                    typed.source,
                    aq_selection::ObservationSource::Operator { .. }
                )
                && typed.received_at_ms >= checked_at
                && typed.expires_at_ms.checked_sub(typed.received_at_ms) == Some(30000),
            "new actual owner observation timing/scope"
        );
        // Tie the explicitly borrowed fixture pool to the real protected owner
        // operation. The caller supplies state and pool from the same Env.
        let retained: Value = sqlx::query_scalar(
            "SELECT response FROM intelligence_operations WHERE operation_id=$1",
        )
        .bind(operation)
        .fetch_one(pool)
        .await?;
        ensure!(
            retained == observed,
            "actual fixture pool/owner operation join"
        );
        if let Some((operation, previous)) = &self.last {
            let retained: Value = sqlx::query_scalar(
                "SELECT response FROM intelligence_operations WHERE operation_id=$1",
            )
            .bind(operation)
            .fetch_one(pool)
            .await?;
            ensure!(
                &retained == previous,
                "prior observation operation reply changed"
            );
        }
        self.sequence = sequence;
        self.last = Some((operation, observed.clone()));
        self.subject = Some(subject);
        Ok(
            json!({"scope":"finite_local_responses_peer_only","observation":observed,"checked_at_ms":checked_at,"journal_digest":journal_digest,"actual_image_count":5,"material_revalidated":true,"native_registration_revalidated":true,"inventory_revalidated":true,"finite_peer_listening":true,"provider_availability_claim":false,"selection_rewritten":false,"ttl_ms":30000,"observation_max_age_ms":5000}),
        )
    }
}
