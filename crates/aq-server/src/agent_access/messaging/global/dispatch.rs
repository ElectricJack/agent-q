//! F1b: one bounded supervisor turn per reservation. The dispatch intent and the
//! exact host input are durable before any host handoff; plans carry only refs.
use super::{Result, db, reservations, unavailable};
use crate::error::AppError;
use aq_central_authority::{
    CentralPlan, CentralSubject, Digest, DispatchState, Event, HandoffDisposition, MailBatch,
    MailRef, Phase, Run, SessionIdentity,
    owner::{EvidenceVerifier, OwnerError, OwnerResult, SessionOwner, Snapshot},
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

const MAX_INPUT: usize = 1048576;

/// One immutable turn: an ordered batch of refs and the exact bytes the host stages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub batch: MailBatch,
    pub dispatch_id: Uuid,
    pub host_operation_id: Uuid,
    pub input: Vec<u8>,
}

/// Deterministic UTF-8 turn text: a header, then each message in batch order.
pub fn render_input(conversation: Uuid, members: &[(MailRef, String)]) -> Vec<u8> {
    let mut s = format!(
        "aq global supervisor turn: conversation {conversation}, {} owner message(s)\n",
        members.len()
    );
    for (m, body) in members {
        s.push_str(&format!(
            "--- message {} cursor {} ---\n{body}\n",
            m.message_id, m.cursor
        ));
    }
    s.into_bytes()
}

fn refused() -> AppError {
    AppError::Conflict("supervisor dispatch does not match the recorded turn".into())
}

/// Read-only access to central session state; every mutation path refuses.
pub(crate) struct ReadOnly;
#[async_trait::async_trait]
impl EvidenceVerifier for ReadOnly {
    async fn authorize_plan(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: &CentralPlan,
    ) -> OwnerResult<()> {
        Err(OwnerError::Unsupported)
    }
    async fn verify_event(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: &Run,
        _: &Event,
    ) -> OwnerResult<()> {
        Err(OwnerError::Unsupported)
    }
}
pub(crate) fn sessions(pool: &PgPool) -> SessionOwner<ReadOnly> {
    SessionOwner::new(pool.clone(), ReadOnly)
}
pub(crate) fn active(s: &Snapshot) -> bool {
    matches!(
        s.run.phase(),
        Phase::StartCommitted | Phase::RunningReported
    )
}
pub(super) fn session_id(plan: &CentralPlan) -> Option<Uuid> {
    match plan.session {
        SessionIdentity::Central { session_id, .. } => Some(session_id),
        _ => None,
    }
}

/// Records (or returns the already recorded) turn for one awaiting reservation.
/// Each generation has one immutable startup turn; later reservations need a
/// successor generation. A consumed turn is retained for exact lookup/replay,
/// never retargeted to a new generation or session.
pub async fn prepare_turn(
    pool: &PgPool,
    actor: Uuid,
    reservation: Uuid,
    generation: u64,
    session_id: Uuid,
    at_ms: u64,
) -> Result<Turn> {
    let g = i64::try_from(generation)
        .ok()
        .filter(|g| (1..=1024).contains(g))
        .ok_or_else(refused)?;
    let at = i64::try_from(at_ms)
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(unavailable)?;
    if session_id.is_nil() {
        return Err(refused());
    }
    let mut tx = pool.begin().await.map_err(db)?;
    let view = reservations::load(&mut tx, actor, reservation).await?;
    let existing = sqlx::query(
        "SELECT * FROM central_supervisor_dispatches WHERE actor_id=$1 AND reservation_id=$2",
    )
    .bind(actor)
    .bind(reservation)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db)?;
    if let Some(row) = existing {
        if row.get::<i64, _>("generation") != g
            || row.get::<Uuid, _>("session_id") != session_id
            || row.get::<String, _>("batch_digest") != view.batch_digest.as_str()
        {
            return Err(refused());
        }
        let input: Vec<u8> = row.get("input_bytes");
        if Digest::of(&input).as_str() != row.get::<String, _>("input_digest") {
            return Err(unavailable());
        }
        tx.commit().await.map_err(db)?;
        return Ok(Turn {
            batch: view.batch,
            dispatch_id: row.get("dispatch_id"),
            host_operation_id: row.get("host_operation_id"),
            input,
        });
    }
    if view.state != reservations::ReservationState::AwaitingPreparedPlan
        || view.batch.digest().ok().as_ref() != Some(&view.batch_digest)
    {
        return Err(refused());
    }
    let bodies: Vec<(Uuid, String)> = sqlx::query("SELECT m.message_id,m.body FROM central_supervisor_mail_members b JOIN mailbox_messages m ON m.message_id=b.message_id WHERE b.reservation_id=$1 ORDER BY b.ordinal LIMIT 17")
        .bind(reservation)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        .into_iter()
        .map(|r| (r.get("message_id"), r.get("body")))
        .collect();
    if bodies.len() != view.batch.messages.len() {
        return Err(unavailable());
    }
    let mut members = Vec::with_capacity(bodies.len());
    for (m, (id, body)) in view.batch.messages.iter().zip(bodies) {
        if m.message_id != id {
            return Err(unavailable());
        }
        members.push((m.clone(), body));
    }
    let input = render_input(view.conversation_id, &members);
    if input.len() > MAX_INPUT {
        return Err(refused());
    }
    let (dispatch_id, host_operation_id) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO central_supervisor_dispatches(dispatch_id,host_operation_id,actor_id,reservation_id,generation,session_id,batch_digest,input_digest,input_bytes,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(dispatch_id).bind(host_operation_id).bind(actor).bind(reservation).bind(g).bind(session_id)
        .bind(view.batch_digest.as_str()).bind(Digest::of(&input).as_str()).bind(&input).bind(at)
        .execute(&mut *tx).await.map_err(|_| refused())?;
    tx.commit().await.map_err(db)?;
    Ok(Turn {
        batch: view.batch,
        dispatch_id,
        host_operation_id,
        input,
    })
}

/// Plan authorization: the plan and its startup bytes must be exactly the recorded turn.
pub async fn authorize_dispatch(
    tx: &mut Transaction<'_, Postgres>,
    plan: &CentralPlan,
    startup_digest: &Digest,
) -> Result<()> {
    let CentralSubject::CentralConversation {
        actor_id,
        conversation_id,
        ..
    } = &plan.subject;
    let row = sqlx::query("SELECT * FROM central_supervisor_dispatches WHERE dispatch_id=$1")
        .bind(plan.dispatch_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db)?
        .ok_or_else(refused)?;
    let reservation: Uuid = row.get("reservation_id");
    let batch_digest = plan.batch.digest().map_err(|_| refused())?;
    if row.get::<Uuid, _>("host_operation_id") != plan.host_operation_id
        || row.get::<Uuid, _>("actor_id") != *actor_id
        || row.get::<i64, _>("generation") != plan.generation as i64
        || Some(row.get::<Uuid, _>("session_id")) != session_id(plan)
        || row.get::<String, _>("batch_digest") != batch_digest.as_str()
        || row.get::<String, _>("input_digest") != startup_digest.as_str()
    {
        return Err(refused());
    }
    let view = reservations::load(tx, *actor_id, reservation).await?;
    if view.batch != plan.batch
        || view.batch_digest != batch_digest
        || view.conversation_id != *conversation_id
    {
        return Err(refused());
    }
    Ok(())
}

/// Dispatch and reply are live while some actor's head generation is active.
pub async fn live(pool: &PgPool) -> bool {
    let Ok(actors) = sqlx::query_scalar::<_, Uuid>(
        "SELECT actor_id FROM central_session_heads WHERE current_generation>0 LIMIT 8",
    )
    .fetch_all(pool)
    .await
    else {
        return false;
    };
    let owner = sessions(pool);
    for actor in actors {
        if let Ok(Some(s)) = owner.inspect(actor).await
            && active(&s)
        {
            return true;
        }
    }
    false
}

/// The owner-visible host delivery state for a dispatch's generation.
pub(super) fn delivery(state: Option<&DispatchState>) -> &'static str {
    match state {
        None | Some(DispatchState::NotAttempted) => "not_attempted",
        Some(DispatchState::IntentCommitted) => "pending",
        Some(DispatchState::Unknown { .. }) => "delivery_unknown",
        Some(DispatchState::Resolved { evidence }) => match evidence.disposition {
            HandoffDisposition::HandedOff => "delivered",
            HandoffDisposition::NotHandedOff => "not_delivered",
        },
    }
}
