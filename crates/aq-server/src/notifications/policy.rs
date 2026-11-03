//! F4 automatic owner notifications. Producers report attention episodes; the
//! policy decides whether a logical message is due and submits it through the
//! notification store. Lock order: policy advisory lock -> policy tables; the
//! store's own transaction runs only after the policy transaction commits.
//!
//! Episodes are keyed by kind + project + subject: one notification when an
//! episode opens, none again until it resolves and reopens. Message and
//! operation IDs derive from the episode generation, and the exact request is
//! frozen before submission, so a replayed source event re-submits the same
//! request and the store answers from its receipt instead of creating a second
//! intent. Delivery outcomes never feed back into this policy (N4).
use super::{Error, Result, persistence::Service};
use aq_lifecycle_api::Scope;
use aq_notifications::SendRequest;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

/// Flood window length and the individual notifications allowed per window and
/// project; later episodes in the window collapse into one counted summary.
pub const FLOOD_WINDOW_MS: u64 = 60_000;
pub const FLOOD_INDIVIDUAL_LIMIT: i32 = 3;
/// Graph label marking a node as a milestone gate (no v2 marker existed).
pub const MILESTONE_GATE_LABEL: &str = "aq:milestone-gate";
const TTL_MS: u64 = 86_400_000;
const MAX_EPISODES: i64 = 4096;
const MAX_EVENTS: i64 = 8192;
const MAX_WINDOWS: i64 = 1024;
const TITLE_CHARS: usize = 240;
const SUBJECT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Kind {
    TaskFailedOrBlocked,
    DecisionNeeded,
    /// Event kind defined here and emitted by F5 when an agent asks the owner.
    QuestionAsked,
    /// Attempt failure reason `credential_invalid` (C6b); subject = profile ID.
    CredentialInvalid,
    /// Subject = node ID of a node labelled [`MILESTONE_GATE_LABEL`].
    MilestoneGateReady,
    /// Subject = worker/node ID from worker presence.
    WorkerOffline,
}
impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::TaskFailedOrBlocked,
        Kind::DecisionNeeded,
        Kind::QuestionAsked,
        Kind::CredentialInvalid,
        Kind::MilestoneGateReady,
        Kind::WorkerOffline,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::TaskFailedOrBlocked => "task.failed_or_blocked",
            Kind::DecisionNeeded => "decision.needed",
            Kind::QuestionAsked => "question.asked",
            Kind::CredentialInvalid => "credential.invalid",
            Kind::MilestoneGateReady => "milestone.gate_ready",
            Kind::WorkerOffline => "worker.offline",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Kind::TaskFailedOrBlocked => "Task failed or blocked",
            Kind::DecisionNeeded => "Decision needed",
            Kind::QuestionAsked => "Question asked",
            Kind::CredentialInvalid => "Credential invalid",
            Kind::MilestoneGateReady => "Milestone gate ready",
            Kind::WorkerOffline => "Worker offline",
        }
    }
    /// Maps an attempt failure reason to the episode kind it opens, if any.
    pub fn from_failure_reason(reason: &str) -> Option<Kind> {
        (reason == "credential_invalid").then_some(Kind::CredentialInvalid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Transition {
    Opened,
    Resolved,
}

/// One confirmed source observation. `title` is a short label chosen by the
/// producer (never a transcript); it is sanitised and truncated here.
#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub source_event_id: Uuid,
    pub kind: Kind,
    pub subject: String,
    pub scope: Scope,
    pub transition: Transition,
    pub title: String,
}

#[derive(Clone)]
pub struct Policy {
    service: Service,
}

fn derived_id(parts: &[&str]) -> Uuid {
    let hex = crate::tokens::verifier(&parts.join("\u{1f}"));
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    // RFC 9562 version 8 (custom) layout with the RFC variant bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn sanitize(text: &str, max_chars: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let mut cut: String = trimmed.chars().take(max_chars - 1).collect();
        cut.push('…');
        cut
    }
}

async fn owner(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid> {
    sqlx::query_scalar("SELECT id FROM users WHERE role='owner' ORDER BY created_at,id LIMIT 1")
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::NotFound)
}

async fn count(tx: &mut Transaction<'_, Postgres>, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(&mut **tx)
        .await?)
}

impl Policy {
    pub fn new(service: Service) -> Self {
        Self { service }
    }
    pub fn service(&self) -> &Service {
        &self.service
    }

    async fn submit(&self, request: Option<Value>, mut response: Value) -> Result<Value> {
        let Some(request) = request else {
            return Ok(response);
        };
        let request: SendRequest = serde_json::from_value(request)?;
        let actor = {
            let mut tx = self.service.pool.begin().await?;
            let id = owner(&mut tx).await?;
            tx.commit().await?;
            id
        };
        let stored = self.service.enqueue(actor, request).await?;
        response["external_state"] = stored["external_state"].clone();
        Ok(response)
    }

    /// Records one source observation and submits the notification it makes
    /// due, if any. Replaying a source event returns the same outcome.
    pub async fn observe(&self, event: Event) -> Result<Value> {
        let subject = sanitize(&event.subject, SUBJECT_BYTES);
        if event.source_event_id.is_nil()
            || subject.is_empty()
            || subject.len() > SUBJECT_BYTES
            || event.scope.project_id.is_nil()
            || event.scope.workspace_id.is_nil()
        {
            return Err(Error::Invalid);
        }
        let hash = super::persistence::digest(&("policy-event/1", &event))?;
        let mut tx = self.service.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(x'61716e6f74696679'::bigint)")
            .execute(&mut *tx)
            .await?;
        let replay = sqlx::query(
            "SELECT request_digest,response,request FROM notification_policy_events WHERE source_event_id=$1",
        )
        .bind(event.source_event_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = replay {
            tx.commit().await?;
            if row.get::<String, _>("request_digest") != hash {
                return Err(Error::Conflict);
            }
            return self.submit(row.get("request"), row.get("response")).await;
        }
        if count(&mut tx, "notification_policy_events").await? >= MAX_EVENTS {
            return Err(Error::Overloaded);
        }
        let now = self.service.now()?;
        let key = format!(
            "{}:{}:{}",
            event.kind.as_str(),
            event.scope.project_id,
            subject
        );
        let current = sqlx::query(
            "SELECT generation,open,workspace_id FROM notification_episodes WHERE episode_key=$1 FOR UPDATE",
        )
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
        let (generation, open) = match &current {
            Some(row) => {
                if row.get::<Uuid, _>("workspace_id") != event.scope.workspace_id {
                    return Err(Error::Conflict);
                }
                (row.get::<i64, _>("generation"), row.get::<bool, _>("open"))
            }
            None => (0, false),
        };
        let mut request = None;
        let response = match (event.transition, open) {
            (Transition::Opened, true) => {
                json!({"episode_key":key,"generation":generation,"outcome":"already_open"})
            }
            (Transition::Resolved, false) => {
                json!({"episode_key":key,"generation":generation,"outcome":"not_open"})
            }
            (Transition::Resolved, true) => {
                sqlx::query("UPDATE notification_episodes SET open=FALSE,resolved_at_ms=$2 WHERE episode_key=$1")
                    .bind(&key).bind(now as i64).execute(&mut *tx).await?;
                json!({"episode_key":key,"generation":generation,"outcome":"resolved"})
            }
            (Transition::Opened, false) => {
                if current.is_none()
                    && count(&mut tx, "notification_episodes").await? >= MAX_EPISODES
                {
                    return Err(Error::Overloaded);
                }
                let generation = generation + 1;
                let window = (now / FLOOD_WINDOW_MS * FLOOD_WINDOW_MS) as i64;
                let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_flood_windows WHERE project_id=$1 AND workspace_id=$2 AND window_start_ms=$3)")
                    .bind(event.scope.project_id).bind(event.scope.workspace_id).bind(window).fetch_one(&mut *tx).await?;
                if !exists && count(&mut tx, "notification_flood_windows").await? >= MAX_WINDOWS {
                    return Err(Error::Overloaded);
                }
                let notified: i32 = sqlx::query_scalar("INSERT INTO notification_flood_windows(project_id,workspace_id,window_start_ms) VALUES($1,$2,$3) ON CONFLICT(project_id,workspace_id,window_start_ms) DO UPDATE SET notified=notification_flood_windows.notified RETURNING notified")
                    .bind(event.scope.project_id).bind(event.scope.workspace_id).bind(window).fetch_one(&mut *tx).await?;
                let outcome = if notified < FLOOD_INDIVIDUAL_LIMIT {
                    let generation_text = generation.to_string();
                    let summary = format!(
                        "AQ attention: {} - {} ({})",
                        event.kind.label(),
                        sanitize(&event.title, TITLE_CHARS),
                        subject
                    );
                    let send = SendRequest {
                        operation_id: derived_id(&["aq-notify-op/1", &key, &generation_text]),
                        message_id: derived_id(&["aq-notify-msg/1", &key, &generation_text]),
                        revision: 1,
                        scope: event.scope.clone(),
                        recipient_user_id: owner(&mut tx).await?,
                        summary,
                        ttl_ms: TTL_MS,
                    };
                    request = Some(serde_json::to_value(&send)?);
                    sqlx::query("UPDATE notification_flood_windows SET notified=notified+1 WHERE project_id=$1 AND workspace_id=$2 AND window_start_ms=$3")
                        .bind(event.scope.project_id).bind(event.scope.workspace_id).bind(window).execute(&mut *tx).await?;
                    "notified"
                } else {
                    sqlx::query("UPDATE notification_flood_windows SET collapsed=collapsed+1,kinds=CASE WHEN $4=ANY(kinds) THEN kinds ELSE array_append(kinds,$4) END WHERE project_id=$1 AND workspace_id=$2 AND window_start_ms=$3")
                        .bind(event.scope.project_id).bind(event.scope.workspace_id).bind(window).bind(event.kind.as_str()).execute(&mut *tx).await?;
                    "collapsed"
                };
                sqlx::query("INSERT INTO notification_episodes(episode_key,kind,subject,project_id,workspace_id,generation,open,opened_at_ms,outcome,request) VALUES($1,$2,$3,$4,$5,$6,TRUE,$7,$8,$9) ON CONFLICT(episode_key) DO UPDATE SET generation=EXCLUDED.generation,open=TRUE,opened_at_ms=EXCLUDED.opened_at_ms,resolved_at_ms=NULL,outcome=EXCLUDED.outcome,request=EXCLUDED.request")
                    .bind(&key).bind(event.kind.as_str()).bind(&subject).bind(event.scope.project_id).bind(event.scope.workspace_id).bind(generation).bind(now as i64).bind(outcome).bind(&request).execute(&mut *tx).await?;
                let mut response =
                    json!({"episode_key":key,"generation":generation,"outcome":outcome});
                if let Some(r) = &request {
                    response["message_id"] = r["message_id"].clone();
                }
                response
            }
        };
        sqlx::query("INSERT INTO notification_policy_events(source_event_id,request_digest,response,request) VALUES($1,$2,$3,$4)")
            .bind(event.source_event_id).bind(&hash).bind(&response).bind(&request).execute(&mut *tx).await?;
        tx.commit().await?;
        self.submit(request, response).await
    }

    /// Submits one counted summary per closed flood window that collapsed
    /// episodes. The summary request is frozen before submission so a retry
    /// after a crash re-submits the identical request. Returns windows flushed.
    pub async fn flush(&self) -> Result<usize> {
        let now = self.service.now()? as i64;
        let mut tx = self.service.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(x'61716e6f74696679'::bigint)")
            .execute(&mut *tx)
            .await?;
        let rows = sqlx::query("SELECT project_id,workspace_id,window_start_ms,collapsed,kinds,summary FROM notification_flood_windows WHERE collapsed>0 AND NOT submitted AND window_start_ms+$1<=$2 ORDER BY window_start_ms LIMIT 8 FOR UPDATE")
            .bind(FLOOD_WINDOW_MS as i64).bind(now).fetch_all(&mut *tx).await?;
        if rows.is_empty() {
            return Ok(0);
        }
        let recipient = owner(&mut tx).await?;
        let mut pending = Vec::new();
        for row in rows {
            let project: Uuid = row.get("project_id");
            let workspace: Uuid = row.get("workspace_id");
            let start: i64 = row.get("window_start_ms");
            let request = match row.get::<Option<Value>, _>("summary") {
                Some(v) => v,
                None => {
                    let collapsed: i32 = row.get("collapsed");
                    let kinds: Vec<String> = row.get("kinds");
                    let start_text = start.to_string();
                    let scope = project.to_string();
                    let send = SendRequest {
                        operation_id: derived_id(&["aq-flood-op/1", &scope, &start_text]),
                        message_id: derived_id(&["aq-flood-msg/1", &scope, &start_text]),
                        revision: 1,
                        scope: Scope {
                            project_id: project,
                            workspace_id: workspace,
                        },
                        recipient_user_id: recipient,
                        summary: format!(
                            "AQ attention: {collapsed} more events need attention in one minute ({}). Open AQ to review.",
                            kinds.join(", ")
                        ),
                        ttl_ms: TTL_MS,
                    };
                    let v = serde_json::to_value(&send)?;
                    sqlx::query("UPDATE notification_flood_windows SET summary=$4 WHERE project_id=$1 AND workspace_id=$2 AND window_start_ms=$3")
                        .bind(project).bind(workspace).bind(start).bind(&v).execute(&mut *tx).await?;
                    v
                }
            };
            pending.push((project, workspace, start, request));
        }
        tx.commit().await?;
        let mut flushed = 0;
        for (project, workspace, start, request) in pending {
            let request: SendRequest = serde_json::from_value(request)?;
            // A failed submission leaves the window pending for the next tick.
            if self.service.enqueue(recipient, request).await.is_ok() {
                sqlx::query("UPDATE notification_flood_windows SET submitted=TRUE WHERE project_id=$1 AND workspace_id=$2 AND window_start_ms=$3")
                    .bind(project).bind(workspace).bind(start).execute(&self.service.pool).await?;
                flushed += 1;
            }
        }
        Ok(flushed)
    }
}
