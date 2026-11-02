//! Disposable worker-view retirement, followed by central-only source evidence.
//! No grant, stop, receipt, source confirmation or release is fabricated here.
#![allow(dead_code)] // Composed fixture owns lifecycle and includes this helper.
use super::{
    actual_git_owner::{self, Owner as GitOwner},
    codex_runner_process::RunnerProcess,
    native_result_settlement::{PROOF_BLOB, PROOF_BYTES, PROOF_PATH},
    native_runner_transport::Transferred,
};
use anyhow::{Context, Result, ensure};
use aq_admission::Digest;
use aq_server::{
    AppState,
    profiled_execution::results,
    source_retention::{self, RecordedSourceConfirmation},
};
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    fs::File,
    io::Read,
    path::Path,
    process::Stdio,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

pub struct B1Pins {
    pub report_id: Uuid,
    pub source_operation: Uuid,
}
struct Pins {
    report_id: Uuid,
    source_operation: Uuid,
    report_digest: Digest,
    confirmation_digest: Digest,
    journal_digest: Digest,
    source: RecordedSourceConfirmation,
}
/// Opaque fixture prerequisites, not serializable/reconstructible authority.
pub struct RetirementReady {
    view: Weak<Transferred>,
    pins: Pins,
}
/// Only produced after consuming/dropping the final exact worker-view owner.
/// Contains central owner records and digests, never a PrivateState or worker FD.
pub struct RetiredSource {
    pins: Pins,
}

fn canonical(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    value.sort_all_objects();
    Ok(serde_json::to_vec(&value)?)
}
fn confirmation_digest(source: &RecordedSourceConfirmation) -> Result<Digest> {
    Ok(Digest::of(&canonical(
        &json!({"manifest":source.manifest,"receipt":source.receipt,"acknowledgement":source.acknowledgement}),
    )?))
}
fn owner_error(_: aq_server::error::AppError) -> anyhow::Error {
    anyhow::anyhow!("actual retained owner prerequisites unavailable")
}
fn central_owner(state: &AppState, central: &GitOwner) -> Result<()> {
    let actual = state
        .source_retention
        .as_ref()
        .context("actual central source runtime absent")?;
    ensure!(
        actual.config.git_root == central.root && actual.config.git_socket == central.socket,
        "exact configured central source owner required"
    );
    Ok(())
}

/// Read the actual still-available worker journal and join it to the actual B1,
/// paired nominal terminal owners, ECHILD proof and confirmed/current custody.
/// Wrapper exit is observed, never interpreted as a successful harness outcome:
/// an intentional response-loss recovery can retain the original nonzero exit.
pub async fn capture(
    state: &AppState,
    pool: &sqlx::PgPool,
    central: &GitOwner,
    worker: &mut RunnerProcess,
    view: &Arc<Transferred>,
    ids: B1Pins,
) -> Result<RetirementReady> {
    central_owner(state, central)?;
    ensure!(
        worker.finished()? && view.sender_exited()?,
        "owned wrapper/provisioner exit must precede worker-view retirement"
    );
    ensure!(
        !ids.report_id.is_nil() && !ids.source_operation.is_nil(),
        "actual B1/source identities required"
    );
    let journal_bytes = view
        .state
        .read(Path::new("journal/journal.json"), 4 * 1024 * 1024)?;
    let journal: Value = serde_json::from_slice(&journal_bytes)?;
    ensure!(
        journal["state"]["format"] == 12
            && journal["digest"] == json!(Digest::of(&canonical(&journal["state"])?)),
        "actual worker journal digest/format required"
    );
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='1s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    let record = results::load_locked(state, &mut tx, ids.report_id)
        .await
        .map_err(owner_error)?;
    ensure!(
        record.is_current_report_head,
        "actual current B1 required before retirement"
    );
    let profile = match record.origin {
        results::RecordedOrigin::PhaseExecutionV2 => "phase_terminal",
        results::RecordedOrigin::PhaseRefreshExecutionV1 => "refresh_terminal",
        results::RecordedOrigin::LegacyPrepareV1 => {
            anyhow::bail!("native phase owner required, no legacy proof substitution")
        }
    };
    let attempt_id = record.report.binding.attempt_id;
    let attempt = &journal["state"]["attempts"][attempt_id.to_string()];
    let host = &attempt["native_host"];
    ensure!(
        journal["state"]["identity"] == json!(record.report.identity)
            && attempt["result"]["report"]["request"] == json!(record.report)
            && attempt["result"]["receipt"] == json!(record.receipt),
        "same-attempt actual journal/B1 association required"
    );
    for (table, field) in [
        ("profiled_started_receipts", "started"),
        ("profiled_stopped_receipts", "stopped"),
    ] {
        // Fixed table names above, never caller SQL or protocol names.
        let row = sqlx::query(&format!("SELECT node_id,request_bytes,phase_envelope_bytes,phase_ack_bytes FROM {table} WHERE attempt_id=$1 FOR SHARE"))
            .bind(attempt_id).fetch_one(&mut *tx).await?;
        let request = row
            .get::<Option<Vec<u8>>, _>("phase_envelope_bytes")
            .context("actual nominal terminal request absent")?;
        let ack = row
            .get::<Option<Vec<u8>>, _>("phase_ack_bytes")
            .context("actual paired terminal ACK absent")?;
        ensure!(
            row.get::<Uuid, _>("node_id") == record.report.identity.node_id
                && canonical(&host[profile][field]["request"])? == request
                && canonical(&host[profile][field]["receipt"])? == ack,
            "actual same-attempt paired terminal publication/ACK required"
        );
        if field == "stopped" {
            let stopped =
                aq_admission::stopped::decode_request(&row.get::<Vec<u8>, _>("request_bytes"))?;
            ensure!(
                stopped.proof.observation.kernel_wait_echild
                    && host["running"].is_object()
                    && host["stopped"] == json!(stopped.proof)
                    && stopped.evidence.binding == record.report.binding
                    && canonical(&stopped.evidence)? == canonical(&record.report.stop)?,
                "actual ECHILD/Stopped proof must match the recorded owner"
            );
        }
    }
    let source = source_retention::load_confirmed_locked(&mut tx, ids.source_operation, &record)
        .await
        .map_err(owner_error)?;
    ensure!(
        source.receipt.source != source.manifest.source.base,
        "actual changed checkpoint required before retirement"
    );
    let report_digest = record.receipt.report_digest;
    let digest = confirmation_digest(&source)?;
    tx.commit().await?;
    source_retention::result_retention::inspect_current(state, &source)
        .await
        .map_err(owner_error)?
        .validate_current(state, &source)
        .map_err(owner_error)?;
    Ok(RetirementReady {
        view: Arc::downgrade(view),
        pins: Pins {
            report_id: ids.report_id,
            source_operation: ids.source_operation,
            report_digest,
            confirmation_digest: digest,
            journal_digest: Digest::of(&journal_bytes),
            source,
        },
    })
}
impl RetirementReady {
    /// Caller has already stopped/joined router/provider tasks and dropped every
    /// connection/callback owner. Refusal restores the caller's forensic Arc.
    /// No worker file is opened here, and no PrivateState exists in the result.
    pub fn retire(self, view: &mut Option<Arc<Transferred>>) -> Result<RetiredSource> {
        let actual = view.as_ref().context("actual worker view absent")?;
        ensure!(
            Weak::ptr_eq(&self.view, &Arc::downgrade(actual)) && actual.sender_exited()?,
            "exact captured exited worker view required"
        );
        ensure!(
            Arc::strong_count(actual) == 1,
            "worker callback/view owners remain; preserve the view"
        );
        let actual = view.take().context("actual worker view absent")?;
        let actual = match Arc::try_unwrap(actual) {
            Ok(actual) => actual,
            Err(actual) => {
                *view = Some(actual);
                anyhow::bail!("worker view retirement raced; forensic owner restored");
            }
        };
        drop(actual); // Final PrivateState File, socket and sender-pidfd owner.
        Ok(RetiredSource { pins: self.pins })
    }
}
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= maximum as u64,
        "bounded actual central export file required"
    );
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= maximum && bytes.len() as u64 == metadata.len(),
        "actual central export file changed"
    );
    Ok(bytes)
}
/// Raw builtin Git object read: unlike the shared text helper, never trim bytes.
async fn raw_blob(root: &Path, blob: &str) -> Result<Vec<u8>> {
    let mut child = tokio::process::Command::new(std::env::var("AQ_TEST_GIT_EXECUTABLE")?)
        .args(["--no-pager", "cat-file", "blob", blob])
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .context("actual Git blob output unavailable")?;
    let read = async {
        let mut bytes = Vec::new();
        (&mut stdout).take(33).read_to_end(&mut bytes).await?;
        ensure!(
            bytes.len() <= 32,
            "fixed actual Git proof blob exceeds bound"
        );
        ensure!(
            child.wait().await?.success(),
            "actual Git proof blob read refused"
        );
        Ok::<_, anyhow::Error>(bytes)
    };
    let result = match tokio::time::timeout(Duration::from_secs(10), read).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!("actual Git blob deadline exceeded")),
    };
    if result.is_err() && child.try_wait()?.is_none() {
        child.start_kill()?;
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .context("actual Git blob cleanup unknown")??;
    }
    result
}
fn exact_proof(bytes: &[u8]) -> Result<Digest> {
    ensure!(
        bytes == PROOF_BYTES && bytes.len() == 31,
        "surviving central source must contain exact31 proof bytes"
    );
    Ok(Digest::of(bytes))
}

/// Central-only verification. Deliberately NO PrivateState, Transferred, worker
/// path, private-file bytes or fallback can be supplied after retirement.
pub async fn verify_after_retirement(
    state: &AppState,
    pool: &sqlx::PgPool,
    central: &GitOwner,
    retired: &RetiredSource,
) -> Result<Value> {
    central_owner(state, central)?;
    let pins = &retired.pins;
    let mut tx = pool.begin().await?;
    let record = results::load_locked(state, &mut tx, pins.report_id)
        .await
        .map_err(owner_error)?;
    ensure!(
        record.receipt.report_digest == pins.report_digest,
        "original B1 digest changed after worker-view retirement"
    );
    let source = source_retention::load_confirmed_locked(&mut tx, pins.source_operation, &record)
        .await
        .map_err(owner_error)?;
    ensure!(
        confirmation_digest(&source)? == pins.confirmation_digest,
        "confirmed central source identity changed after retirement"
    );
    tx.commit().await?;
    source_retention::result_retention::inspect_current(state, &source)
        .await
        .map_err(owner_error)?
        .validate_current(state, &source)
        .map_err(owner_error)?;
    let repository = source.receipt.repository_id;
    let checkpoint = &source.receipt.source;
    let tree = central
        .call(
            repository,
            "tree",
            json!({"oid":checkpoint,"limit":100,"recursive":true}),
        )
        .await;
    ensure!(
        canonical(&tree)?.len() <= 65536 && tree["truncated"] == false,
        "actual central tree must be bounded and complete"
    );
    let entries = tree["entries"]
        .as_array()
        .context("actual central tree entries absent")?;
    let proofs: Vec<_> = entries
        .iter()
        .filter(|e| {
            (e["path"]["encoding"] == "utf8" && e["path"]["value"] == PROOF_PATH)
                || (e["path"]["encoding"] == "hex"
                    && e["path"]["value"] == "6761746530342d70726f6f662e747874")
        })
        .collect();
    ensure!(
        proofs.len() == 1
            && proofs[0]["mode"] == "100644"
            && proofs[0]["type"] == "blob"
            && proofs[0]["oid"] == json!({"algorithm":"sha1","hex":PROOF_BLOB}),
        "surviving central tree differs from exact fixed proof"
    );
    let export=central.call(repository,"artifact.export",json!({"base":source.manifest.source.base,"source":checkpoint,"retention_handle":source.receipt.retention_handle,"confirmed_prerequisites":[]})).await;
    let manifest = &export["manifest"];
    let artifact: Uuid = serde_json::from_value(export["artifact_handle"].clone())?;
    ensure!(
        !artifact.is_nil()
            && manifest["repository_id"] == json!(repository)
            && manifest["binding_generation"] == source.receipt.binding_generation
            && manifest["base"] == json!(source.manifest.source.base)
            && manifest["source"] == json!(checkpoint)
            && manifest["reference_only"] == false,
        "actual central export must preserve original source pins"
    );
    let size = manifest["bundle_size"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 1024 * 1024)
        .context("bounded actual export bundle required")?;
    let artifact_path = central.artifact(&export);
    ensure!(
        serde_json::from_slice::<Value>(&read_bounded(
            &artifact_path.join("manifest.json"),
            65536
        )?)? == *manifest,
        "actual exported manifest differs"
    );
    let bundle = read_bounded(&artifact_path.join("objects.bundle"), 1024 * 1024)?;
    ensure!(
        bundle.len() as u64 == size && manifest["bundle_sha256"] == json!(Digest::of(&bundle)),
        "actual exported bundle digest differs"
    );
    actual_git_owner::verify_pins();
    let verify = tempfile::tempdir()?;
    actual_git_owner::git(verify.path(), &["init"]).await;
    let bundle_path = artifact_path.join("objects.bundle");
    actual_git_owner::git(
        verify.path(),
        &[
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "fetch",
            "--no-tags",
            bundle_path.to_str().context("actual export path UTF8")?,
            &checkpoint.hex,
        ],
    )
    .await;
    let actual_blob = actual_git_owner::git(
        verify.path(),
        &["rev-parse", &format!("{}:{PROOF_PATH}", checkpoint.hex)],
    )
    .await;
    ensure!(
        actual_blob == PROOF_BLOB,
        "fresh exported repository proof blob differs"
    );
    let bytes = match raw_blob(verify.path(), PROOF_BLOB).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _retained = verify.keep();
            return Err(error);
        }
    };
    let digest = exact_proof(&bytes)?;
    Ok(
        json!({"worker_view_retired":true,"proof":"actual central retention/export/fresh Git object read only","report_id":pins.report_id,"source_operation":pins.source_operation,"repository_id":repository,"binding_generation":source.receipt.binding_generation,"checkpoint":checkpoint,"retention_handle":source.receipt.retention_handle,"central_export_artifact":artifact,"proof_blob":PROOF_BLOB,"proof_bytes":bytes.len(),"proof_sha256":digest,"pre_retirement_journal_digest":pins.journal_digest,"current_central_retention_confirmed":true,"worker_private_state_used_after_retirement":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_raw_proof_refuses_trim_equivalent_and_extra_bytes() {
        exact_proof(PROOF_BYTES).unwrap();
        let mut changed = vec![b'\n'];
        changed.extend_from_slice(&PROOF_BYTES[..30]);
        assert_eq!(
            String::from_utf8_lossy(&changed).trim(),
            String::from_utf8_lossy(PROOF_BYTES).trim()
        );
        assert!(exact_proof(&changed).is_err());
        let mut changed = PROOF_BYTES.to_vec();
        changed.push(b'\n');
        assert!(exact_proof(&changed).is_err());
    }
}
