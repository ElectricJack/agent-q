// Included into distinct nominal protocol modules; no DTO family conversion.
pub(in crate::profiled_execution::results) struct Verified {
    pub pin: Pin,
    pub stopped: wire::StoppedRequest,
    pub stopped_ack: wire::StoppedReceipt,
}
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
    history: phase_owner::history::PhaseExecutionHistory,
) -> Result<Verified> {
    let registration =
        native::closure::history::load_locked(tx, history.provenance.registration_id).await?;
    let prepared = aq_admission::granted_closure::JournalCheckpoint {
        revision: history.request.original.journal_revision,
        digest: history.request.original.journal_digest.clone(),
    };
    let context = wire::Context {
        execution: &history.receipt,
        prepare: &history.request,
        accepted: &history.accepted.receipt,
        acceptance: &history.accepted.request,
        proposal: &history.accepted.proposal,
        runtime: &registration.metadata.request.runtime,
        runtime_qualification: &registration.metadata.qualification,
        prepared: &prepared,
    };
    context.validate().map_err(|_| conflict())?;
    if history.receipt.grant.binding != *binding
        || registration.metadata.request.identity.node_id != node
    {
        return Err(conflict());
    }
    let start =
        sqlx::query("SELECT * FROM profiled_started_receipts WHERE attempt_id=$1 FOR SHARE")
            .bind(binding.attempt_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db)?
            .ok_or_else(conflict)?;
    let stop = sqlx::query("SELECT * FROM profiled_stopped_receipts WHERE attempt_id=$1 FOR SHARE")
        .bind(binding.attempt_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db)?
        .ok_or_else(conflict)?;
    let start_bytes = start
        .get::<Option<Vec<u8>>, _>("phase_envelope_bytes")
        .ok_or_else(conflict)?;
    let started = wire::decode_started_request(&start_bytes).map_err(|_| conflict())?;
    let start_ack_bytes = start
        .get::<Option<Vec<u8>>, _>("phase_ack_bytes")
        .ok_or_else(conflict)?;
    let started_ack =
        wire::decode_started_receipt(&start_ack_bytes, &started).map_err(|_| conflict())?;
    let stop_bytes = stop
        .get::<Option<Vec<u8>>, _>("phase_envelope_bytes")
        .ok_or_else(conflict)?;
    let stopped = wire::decode_stopped_request(&stop_bytes).map_err(|_| conflict())?;
    let stop_ack_bytes = stop
        .get::<Option<Vec<u8>>, _>("phase_ack_bytes")
        .ok_or_else(conflict)?;
    let stopped_ack =
        wire::decode_stopped_receipt(&stop_ack_bytes, &stopped).map_err(|_| conflict())?;
    started.validate_for(&context).map_err(|_| conflict())?;
    stopped
        .validate_for(&context, &started)
        .map_err(|_| conflict())?;
    for (row, outer, ack, digest, inner, inner_ack, inner_digest) in [
        (
            &start,
            &start_bytes,
            &start_ack_bytes,
            started.digest().map_err(|_| conflict())?,
            canonical(&started.original)?,
            canonical(&started_ack.original)?,
            started.original.digest().map_err(|_| conflict())?,
        ),
        (
            &stop,
            &stop_bytes,
            &stop_ack_bytes,
            stopped.digest().map_err(|_| conflict())?,
            canonical(&stopped.original)?,
            canonical(&stopped_ack.original)?,
            stopped.original.digest().map_err(|_| conflict())?,
        ),
    ] {
        if row.get::<Uuid, _>("node_id") != node
            || row.get::<String, _>("initial_authority_kind") != "phase_execution_v2"
            || row
                .get::<Option<String>, _>("initial_authority_receipt_digest")
                .as_deref()
                != Some(history.receipt_digest.as_str())
            || row
                .get::<Option<String>, _>("phase_request_digest")
                .as_deref()
                != Some(digest.as_str())
            || row
                .get::<Option<String>, _>("phase_envelope_digest")
                .as_deref()
                != Some(Digest::of(outer).as_str())
            || row.get::<Option<String>, _>("phase_ack_digest").as_deref()
                != Some(Digest::of(ack).as_str())
            || row.get::<Vec<u8>, _>("request_bytes") != inner
            || row.get::<Vec<u8>, _>("receipt_bytes") != inner_ack
            || row.get::<String, _>("request_digest") != inner_digest.as_str()
        {
            return Err(conflict());
        }
    }
    let qualification: ArtifactRef = serde_json::from_value(
        stop.get::<Option<serde_json::Value>, _>("phase_terminal_qualification")
            .ok_or_else(conflict)?,
    )
    .map_err(|_| conflict())?;
    // Qualification was admitted by the actual terminal owner. Historical
    // cleanup neither invents that admission nor requires its current allowlist.
    if start.get::<Option<serde_json::Value>, _>("phase_terminal_qualification")
        != Some(serde_json::to_value(&qualification).map_err(|_| AppError::Internal)?)
        || canonical(&started)? != start_bytes
        || canonical(&started_ack)? != start_ack_bytes
        || canonical(&stopped)? != stop_bytes
        || canonical(&stopped_ack)? != stop_ack_bytes
        || qualification.id.as_str().is_empty()
        || qualification.version.as_str().is_empty()
        || canonical(&stopped.original.proof)? != stop.get::<Vec<u8>, _>("proof_bytes")
        || Digest::of(&canonical(&stopped.original.proof)?).as_str()
            != stop.get::<String, _>("proof_digest")
        || !registration
            .metadata
            .request
            .features
            .contains(&Feature::HostStopEvidenceV1)
        || !registration
            .metadata
            .request
            .features
            .contains(&Feature::DurableHostJournalV1)
    {
        return Err(conflict());
    }
    Ok(Verified {
        pin: Pin {
            kind: KIND,
            execution: Some(history.receipt_digest.as_str().into()),
            stopped_ack_bytes: Some(Digest::of(&stop_ack_bytes).as_str().into()),
        },
        stopped,
        stopped_ack,
    })
}
