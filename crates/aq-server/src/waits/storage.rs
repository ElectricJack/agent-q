use super::*;
use slots::digest;
use sqlx::{Postgres, Row, Transaction};
const ENVELOPE: i64 = 1048576;
fn registration(request: &wire::Request) -> Result<&wire::WaitRegistration> {
    match &request.body.command {
        wire::Command::RegisterWait(v) => Ok(v),
        _ => Err(Error::Unsupported),
    }
}
fn phase(snapshot: &engine::Snapshot) -> &'static str {
    match snapshot.phase() {
        engine::Phase::Installing => "installing",
        engine::Phase::Pending => "pending",
        engine::Phase::RecoveryHeld => "recovery_held",
        engine::Phase::Terminal => "terminal",
    }
}
fn uuid_for(wait: Uuid, label: &str) -> Uuid {
    Uuid::new_v5(&wait, label.as_bytes())
}
fn work_bytes(snapshot: &engine::Snapshot) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&snapshot.pending_work())?)
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Completion {
    wait_id: Uuid,
    outcome_digest: wire::Digest,
    resume: Option<engine::ResumeIntent>,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Cancellation {
    wait_id: Uuid,
    cancellation_id: Uuid,
    anchor: Anchor,
}
pub(super) struct Stored {
    pub observation: Observation,
    pub original: Original,
    pub request_digest: String,
    pub lease: Option<Uuid>,
    pub lease_expires: Option<u64>,
}
impl Owner {
    async fn admission_gates(&self, tx: &mut Transaction<'_, Postgres>, a: &Anchor) -> Result<()> {
        // Global accounting locks occur only on first admission. Progress and
        // terminalization never need these locks or acquire new capacity.
        sqlx::query(
            "SELECT scope_key FROM wait_owner_budgets WHERE scope_key='installation' FOR UPDATE",
        )
        .execute(&mut **tx)
        .await?;
        let scope = format!("project/{}/{}", a.scope.workspace_id, a.scope.project_id);
        sqlx::query("INSERT INTO wait_owner_budgets(scope_key,envelope_limit) VALUES($1,256) ON CONFLICT DO NOTHING").bind(&scope).execute(&mut **tx).await?;
        sqlx::query("SELECT scope_key FROM wait_owner_budgets WHERE scope_key=$1 FOR UPDATE")
            .bind(&scope)
            .execute(&mut **tx)
            .await?;
        sqlx::query("INSERT INTO wait_owner_projects(workspace_id,project_id,revision,budget_scope) VALUES($1,$2,1,$3) ON CONFLICT DO NOTHING")
            .bind(a.scope.workspace_id).bind(a.scope.project_id).bind(scope).execute(&mut **tx).await?;
        self.gates(tx, a, true).await
    }
    pub(super) async fn gates(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        a: &Anchor,
        create: bool,
    ) -> Result<()> {
        let found=sqlx::query("SELECT revision FROM wait_owner_projects WHERE workspace_id=$1 AND project_id=$2 FOR UPDATE")
            .bind(a.scope.workspace_id).bind(a.scope.project_id).fetch_optional(&mut **tx).await?;
        if found.is_none() {
            return Err(Error::NotFound);
        }
        if create {
            sqlx::query("INSERT INTO wait_owner_tasks(workspace_id,project_id,task_id,lineage_id,generation) VALUES($1,$2,$3,$4,1) ON CONFLICT DO NOTHING")
            .bind(a.scope.workspace_id).bind(a.scope.project_id).bind(a.binding.task_id).bind(a.binding.lineage_id).execute(&mut **tx).await?;
        }
        let lineage:Option<Uuid>=sqlx::query_scalar("SELECT lineage_id FROM wait_owner_tasks WHERE workspace_id=$1 AND project_id=$2 AND task_id=$3 FOR UPDATE")
            .bind(a.scope.workspace_id).bind(a.scope.project_id).bind(a.binding.task_id).fetch_optional(&mut **tx).await?;
        if lineage != Some(a.binding.lineage_id) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    async fn graph(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        a: &Anchor,
    ) -> Result<(engine::KnownGraph, GraphProof)> {
        let revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM wait_owner_projects WHERE workspace_id=$1 AND project_id=$2",
        )
        .bind(a.scope.workspace_id)
        .bind(a.scope.project_id)
        .fetch_one(&mut **tx)
        .await?;
        let edges = self.edges_at(tx, a, revision as u64).await?;
        let digest =
            retained_digest(&("aq.wait.known-project-graph.1", &a.scope, revision, &edges))?;
        let edge_count = edges.len();
        Ok((
            engine::KnownGraph {
                scope: a.scope.clone(),
                complete_known_component: true,
                revision: wire::ArtifactRef {
                    id: wire::Text::new(format!(
                        "aq-wait-graph/{}/{}/{}",
                        a.scope.workspace_id, a.scope.project_id, revision
                    ))?,
                    version: wire::Text::new("1")?,
                    digest: digest.clone(),
                },
                edges,
                external_unknown: false,
            },
            GraphProof {
                revision: revision as u64,
                edge_count,
                digest,
            },
        ))
    }
    async fn edges_at(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        a: &Anchor,
        r: u64,
    ) -> Result<Vec<(engine::DependencyKey, engine::DependencyKey)>> {
        let rows=sqlx::query("WITH history AS (SELECT e.waiter_key,e.blocker_key FROM wait_owner_edges e JOIN wait_owner_records w USING(wait_id) WHERE w.workspace_id=$1 AND w.project_id=$2 AND e.activated_revision<=$3 AND (e.retired_revision IS NULL OR e.retired_revision>$3) UNION ALL SELECT 'wait/'||w.wait_id::text AS waiter_key,'task/'||w.project_id::text||'/'||w.task_id::text AS blocker_key FROM wait_owner_records w WHERE w.workspace_id=$1 AND w.project_id=$2 AND w.bridge_created_revision<=$3 AND (w.bridge_retired_revision IS NULL OR w.bridge_retired_revision>$3)) SELECT waiter_key,blocker_key FROM history ORDER BY waiter_key,blocker_key LIMIT 4097")
            .bind(a.scope.workspace_id).bind(a.scope.project_id).bind(r as i64).fetch_all(&mut **tx).await?;

        if rows.len() > 4096 {
            return Err(Error::Capacity);
        }
        fn key(s: &str) -> Result<engine::DependencyKey> {
            let p: Vec<_> = s.split('/').collect();
            match p.as_slice() {
                ["task", project, task] => Ok(engine::DependencyKey::Task {
                    project: project.parse().map_err(|_| Error::Corrupt)?,
                    task: task.parse().map_err(|_| Error::Corrupt)?,
                }),
                ["wait", wait] => Ok(engine::DependencyKey::Wait {
                    wait: wait.parse().map_err(|_| Error::Corrupt)?,
                }),
                _ => Err(Error::Corrupt),
            }
        }
        rows.iter()
            .map(|r| {
                Ok((
                    key(&r.get::<String, _>("waiter_key"))?,
                    key(&r.get::<String, _>("blocker_key"))?,
                ))
            })
            .collect()
    }
    pub async fn register(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        raw: &[u8],
    ) -> Result<Observation> {
        if raw.len() > wire::MAX_REQUEST_BYTES {
            return Err(Error::Capacity);
        }
        let request: wire::Request = serde_json::from_slice(raw)?;
        request.validate()?;
        let reg = registration(&request)?;
        let current = self.preflight(node, attempt, false).await?;
        if reg.scope != current.anchor.scope
            || reg.binding != current.anchor.binding
            || reg.policy != self.policy.reference
        {
            return Err(Error::Authority);
        }
        if !self.fixture_sources
            && reg
                .predicate
                .members()
                .iter()
                .any(|m| !matches!(m.condition, wire::Condition::Timer { .. }))
        {
            return Err(Error::Unsupported);
        }
        let raw = std::str::from_utf8(raw)
            .map_err(|_| Error::Corrupt)?
            .to_owned();
        let hash = digest(raw.as_bytes());
        // A read-only immutable peek chooses lock scope, never supplies authority.
        if let Some(existing_wait) = self.peek_operation(request.body.operation_id).await? {
            if existing_wait != reg.wait_id {
                return Err(Error::Conflict);
            }
            let mut tx = self.transaction().await?;
            self.gates(&mut tx, &current.anchor, false).await?;
            let mut now = self.current(&mut tx, node, attempt, false).await?;
            self.mailbox_head(&mut tx, &now.anchor).await?;
            let stored = self.load(&mut tx, reg.wait_id, true).await?;
            now.refresh(&self.state)?;
            if stored.request_digest != hash
                || stored.original.request != raw
                || stored.original.anchor != now.anchor
            {
                return Err(Error::Conflict);
            }
            stored
                .observation
                .snapshot
                .replay_registration(&request, &now.fence)?;
            tx.commit().await?;
            return Ok(stored.observation);
        }
        let before_admission_anchor = current.anchor.clone();
        let mut tx = self.transaction().await?;
        self.admission_gates(&mut tx, &current.anchor).await?;
        let mut current = self.current(&mut tx, node, attempt, false).await?;
        if current.anchor != before_admission_anchor {
            return Err(Error::Authority);
        }
        if let Some(wait) = sqlx::query_scalar::<_, Uuid>(
            "SELECT wait_id FROM wait_owner_records WHERE operation_id=$1",
        )
        .bind(request.body.operation_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            self.mailbox_head(&mut tx, &current.anchor).await?;
            let stored = self.load(&mut tx, wait, true).await?;
            current.refresh(&self.state)?;
            if wait != reg.wait_id
                || stored.request_digest != hash
                || stored.original.request != raw
                || stored.original.anchor != current.anchor
            {
                return Err(Error::Conflict);
            }
            tx.commit().await?;
            return Ok(stored.observation);
        }
        let active:Option<Uuid>=sqlx::query_scalar("SELECT active_wait_id FROM wait_owner_tasks WHERE workspace_id=$1 AND project_id=$2 AND task_id=$3")
            .bind(reg.scope.workspace_id).bind(reg.scope.project_id).bind(reg.binding.task_id).fetch_one(&mut *tx).await?;
        if active.is_some() {
            return Err(Error::Conflict);
        }
        let (mut graph, proof) = self.graph(&mut tx, &current.anchor).await?;
        // This owner proves only the complete local registry component. Other
        // source owners' dependency closures are not qualified in this slice.
        graph.external_unknown = reg
            .predicate
            .members()
            .iter()
            .any(|m| !matches!(m.condition, wire::Condition::Timer { .. }));
        current.refresh(&self.state)?;
        let ids = engine::StableIds {
            continuation_id: uuid_for(reg.wait_id, "continuation"),
            outcome_id: uuid_for(reg.wait_id, "outcome"),
            resume_intent_id: uuid_for(reg.wait_id, "resume-intent"),
        };
        let plan = engine::Snapshot::begin(
            request.clone(),
            &current.fence,
            self.policy.clone(),
            ids.clone(),
            &graph,
            current.now,
        )?;
        let snapshot = plan.next;
        Self::bounded(&snapshot)?;
        let receipt = RegistrationReceipt {
            contract: "aq.internal.wait-owner.1".into(),
            wait_id: reg.wait_id,
            operation_id: request.body.operation_id,
            request_digest: hash.clone(),
            created_at_ms: snapshot.original_created_at_ms(),
            deadline_ms: snapshot.deadline_ms(),
        };
        let original = Original {
            request: raw,
            anchor: current.anchor.clone(),
            policy: self.policy.clone(),
            ids,
            graph: proof,
        };
        if serde_json::to_vec(&original)?.len() > slots::bound("original")? {
            return Err(Error::Capacity);
        }
        sqlx::query("INSERT INTO wait_owner_records(wait_id,operation_id,request_digest,workspace_id,project_id,task_id,lineage_id,node_id,attempt_id,binding_digest,source_access_digest,policy_digest,source_profile,wait_generation,continuation_id,continuation_generation,outcome_id,resume_intent_id,member_count,envelope_bytes,created_at_ms,deadline_ms,revision,bridge_created_revision,logical_now_ms,next_due_ms,phase,dirty_generation,work_generation,resume_state) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,1,1,$21,$21,'installing',0,1,'none')")
            .bind(reg.wait_id).bind(request.body.operation_id).bind(&hash).bind(reg.scope.workspace_id).bind(reg.scope.project_id).bind(reg.binding.task_id).bind(reg.binding.lineage_id).bind(node.id()).bind(attempt)
            .bind(wire::canonical_digest(&reg.binding)?.as_str()).bind(current.anchor.source_access.as_str()).bind(self.policy.reference.digest.as_str()).bind(if self.fixture_sources{"test_fixture_v1"}else{"local_v1"})
            .bind(reg.wait_generation as i64).bind(original.ids.continuation_id).bind(reg.continuation_generation as i64).bind(original.ids.outcome_id).bind(original.ids.resume_intent_id).bind(reg.predicate.members().len() as i32)
            .bind(ENVELOPE).bind(current.now as i64).bind(snapshot.deadline_ms() as i64).execute(&mut *tx).await?;
        sqlx::query("UPDATE wait_owner_tasks SET active_wait_id=$4,generation=generation+1 WHERE workspace_id=$1 AND project_id=$2 AND task_id=$3")
            .bind(reg.scope.workspace_id).bind(reg.scope.project_id).bind(reg.binding.task_id).bind(reg.wait_id).execute(&mut *tx).await?;
        self.empty_slots(&mut tx, reg.wait_id).await?;
        for (ordinal, member) in reg.predicate.members().iter().enumerate() {
            let (kind, blocker) = match &member.condition {
                wire::Condition::Task { task_id, .. } => (Some("task"), Some(*task_id)),
                wire::Condition::Dependency {
                    depends_on_task_id, ..
                } => (Some("task"), Some(*depends_on_task_id)),
                wire::Condition::WaitOutcome { wait } => (Some("wait"), Some(wait.wait_id)),
                _ => (None, None),
            };
            let subscription = engine::SubscriptionId {
                wait: reg.wait_ref(),
                member_id: member.member_id,
            };
            let owner = serde_json::to_value(member.condition.owner())?
                .as_str()
                .ok_or(Error::Corrupt)?
                .to_owned();
            sqlx::query("INSERT INTO wait_owner_members(wait_id,ordinal,member_id,owner,condition_digest,subscription_digest,blocker_kind,blocker_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                .bind(reg.wait_id).bind(ordinal as i32+1).bind(member.member_id).bind(owner).bind(wire::canonical_digest(&member.condition)?.as_str()).bind(subscription.digest()?.as_str()).bind(kind).bind(blocker).execute(&mut *tx).await?;
            if let (Some(kind), Some(blocker)) = (kind, blocker) {
                let key = if kind == "task" {
                    format!("task/{}/{}", reg.scope.project_id, blocker)
                } else {
                    format!("wait/{blocker}")
                };
                sqlx::query("INSERT INTO wait_owner_edges(wait_id,ordinal,waiter_key,blocker_key,active,activated_revision) VALUES($1,$2,$3,$4,TRUE,1)")
                    .bind(reg.wait_id).bind(ordinal as i32+1).bind(format!("task/{}/{}",reg.scope.project_id,reg.binding.task_id)).bind(key).execute(&mut *tx).await?;
            }
        }
        sqlx::query("INSERT INTO wait_owner_work(wait_id,generation,state,next_due_ms,transition_at_ms) VALUES($1,1,'pending',$2,$2)").bind(reg.wait_id).bind(current.now as i64).execute(&mut *tx).await?;
        for (kind, bytes) in [
            ("original", serde_json::to_vec(&original)?),
            ("registration", serde_json::to_vec(&receipt)?),
            ("snapshot", snapshot.encode()?),
            ("work", work_bytes(&snapshot)?),
        ] {
            self.put_slot(&mut tx, reg.wait_id, &hash, kind, 1, &bytes)
                .await?;
        }
        tx.commit().await?;
        Ok(Observation {
            registration: receipt,
            snapshot,
            retired: false,
        })
    }
    async fn peek_operation(&self, operation: Uuid) -> Result<Option<Uuid>> {
        Ok(
            sqlx::query_scalar("SELECT wait_id FROM wait_owner_records WHERE operation_id=$1")
                .bind(operation)
                .fetch_optional(&self.state.store.pool)
                .await?,
        )
    }
    pub(super) async fn peek(&self, wait: Uuid) -> Result<(Uuid, Uuid)> {
        sqlx::query_as("SELECT node_id,attempt_id FROM wait_owner_records WHERE wait_id=$1")
            .bind(wait)
            .fetch_optional(&self.state.store.pool)
            .await?
            .ok_or(Error::NotFound)
    }
    pub(super) async fn mailbox_head(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        a: &Anchor,
    ) -> Result<()> {
        // Lock only the existing owner head, before registry/member/slot locks.
        // This is an integration seam, not a qualified mailbox proof adapter.
        let key = crate::agent_access::messaging::task_mailbox_key(
            a.scope.workspace_id,
            a.scope.project_id,
            a.binding.task_id,
        )
        .map_err(|_| Error::Corrupt)?;
        sqlx::query("SELECT next_cursor FROM mailbox_heads WHERE mailbox_key=$1 FOR SHARE")
            .bind(key)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(())
    }
    pub(super) fn bounded(snapshot: &engine::Snapshot) -> Result<()> {
        if snapshot.retained_basis_bytes()? > 131072 {
            return Err(Error::Capacity);
        }
        let bytes = snapshot.encode()?;
        if bytes.len()
            > if snapshot.phase() == &engine::Phase::Terminal {
                262144
            } else {
                131072
            }
        {
            return Err(Error::Capacity);
        }
        if let Some(outcome) = snapshot.outcome()
            && serde_json::to_vec(outcome)?.len() > 131072
        {
            return Err(Error::Capacity);
        }
        {
            let (terminal, outcome, resume) = snapshot.reserved_terminal_sizes()?;
            if terminal > 262144 || outcome > 131072 || resume > 8192 {
                return Err(Error::Capacity);
            }
        }
        if work_bytes(snapshot)?.len() > 262144 {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub(super) async fn load(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        wait: Uuid,
        lock: bool,
    ) -> Result<Stored> {
        let sql = if lock {
            "SELECT * FROM wait_owner_records WHERE wait_id=$1 FOR UPDATE"
        } else {
            "SELECT * FROM wait_owner_records WHERE wait_id=$1"
        };
        let row = sqlx::query(sql)
            .bind(wait)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(Error::NotFound)?;
        let source_profile: String = row.get("source_profile");
        if source_profile != "local_v1"
            && !(self.fixture_sources && source_profile == "test_fixture_v1")
        {
            return Err(Error::Unsupported);
        }
        let request_digest: String = row.get("request_digest");
        let original_bytes = self
            .get_slot(tx, wait, &request_digest, "original")
            .await?
            .ok_or(Error::Corrupt)?;
        let original: Original = serde_json::from_slice(&original_bytes)?;
        let request: wire::Request = serde_json::from_str(&original.request)?;
        request.validate()?;
        let reg = registration(&request)?;
        let bytes = self
            .get_slot(tx, wait, &request_digest, "snapshot")
            .await?
            .ok_or(Error::Corrupt)?;
        let snapshot = engine::Snapshot::decode(&bytes)?;
        Self::bounded(&snapshot)?;
        let receipt: RegistrationReceipt = serde_json::from_slice(
            &self
                .get_slot(tx, wait, &request_digest, "registration")
                .await?
                .ok_or(Error::Corrupt)?,
        )?;
        let expected = RegistrationReceipt {
            contract: "aq.internal.wait-owner.1".into(),
            wait_id: wait,
            operation_id: request.body.operation_id,
            request_digest: request_digest.clone(),
            created_at_ms: snapshot.original_created_at_ms(),
            deadline_ms: snapshot.deadline_ms(),
        };
        let a = &original.anchor;
        if digest(original.request.as_bytes()) != request_digest
            || reg.wait_id != wait
            || snapshot.request() != &request
            || snapshot.policy() != &original.policy
            || snapshot.stable_ids() != &original.ids
            || reg.scope != a.scope
            || reg.binding != a.binding
            || receipt != expected
            || original.policy.reference != reg.policy
            || row.get::<Uuid, _>("workspace_id") != a.scope.workspace_id
            || row.get::<Uuid, _>("project_id") != a.scope.project_id
            || row.get::<Uuid, _>("task_id") != a.binding.task_id
            || row.get::<Uuid, _>("lineage_id") != a.binding.lineage_id
            || row.get::<Uuid, _>("node_id") != a.node_id
            || row.get::<Uuid, _>("attempt_id") != a.attempt_id
            || row.get::<Uuid, _>("operation_id") != request.body.operation_id
            || row.get::<String, _>("binding_digest")
                != wire::canonical_digest(&a.binding)?.as_str()
            || row.get::<String, _>("source_access_digest") != a.source_access.as_str()
            || row.get::<String, _>("policy_digest") != original.policy.reference.digest.as_str()
            || row.get::<i64, _>("revision") != snapshot.revision() as i64
            || row.get::<i64, _>("logical_now_ms") != snapshot.logical_now_ms() as i64
            || row.get::<i64, _>("dirty_generation") != snapshot.dirty_generation() as i64
            || row.get::<i64, _>("work_generation") != snapshot.work_generation() as i64
            || row.get::<i64, _>("created_at_ms") != snapshot.original_created_at_ms() as i64
            || row.get::<i64, _>("deadline_ms") != snapshot.deadline_ms() as i64
            || row.get::<i64, _>("next_due_ms") != snapshot.next_due_ms() as i64
            || row.get::<String, _>("phase") != phase(&snapshot)
            || row.get::<i64, _>("wait_generation") != reg.wait_generation as i64
            || row.get::<i64, _>("continuation_generation") != reg.continuation_generation as i64
            || row.get::<Uuid, _>("continuation_id") != original.ids.continuation_id
            || row.get::<Uuid, _>("outcome_id") != original.ids.outcome_id
            || row.get::<Uuid, _>("resume_intent_id") != original.ids.resume_intent_id
            || row.get::<i32, _>("member_count") != reg.predicate.members().len() as i32
        {
            return Err(Error::Corrupt);
        }
        let fence = engine::AuthorityFence {
            scope: a.scope.clone(),
            binding: a.binding.clone(),
            principal: a.principal.clone(),
            source_access: a.source_access.clone(),
            task_cancelled: false,
        };
        snapshot
            .replay_registration(&request, &fence)
            .map_err(|_| Error::Corrupt)?;
        let work = self
            .get_slot(tx, wait, &request_digest, "work")
            .await?
            .ok_or(Error::Corrupt)?;
        if work.as_slice() != work_bytes(&snapshot)?.as_slice() {
            return Err(Error::Corrupt);
        }
        let members=sqlx::query("SELECT member_id,owner,condition_digest,subscription_digest FROM wait_owner_members WHERE wait_id=$1 ORDER BY ordinal").bind(wait).fetch_all(&mut **tx).await?;
        if members.len() != reg.predicate.members().len() {
            return Err(Error::Corrupt);
        }
        for (row, member) in members.iter().zip(reg.predicate.members()) {
            let owner = serde_json::to_value(member.condition.owner())?;
            if row.get::<Uuid, _>("member_id") != member.member_id
                || row.get::<String, _>("owner") != owner.as_str().ok_or(Error::Corrupt)?
                || row.get::<String, _>("condition_digest")
                    != wire::canonical_digest(&member.condition)?.as_str()
                || row.get::<String, _>("subscription_digest")
                    != (engine::SubscriptionId {
                        wait: reg.wait_ref(),
                        member_id: member.member_id,
                    })
                    .digest()?
                    .as_str()
            {
                return Err(Error::Corrupt);
            }
        }
        let edges = self.edges_at(tx, a, original.graph.revision).await?;
        if edges.len() != original.graph.edge_count
            || retained_digest(&(
                "aq.wait.known-project-graph.1",
                &a.scope,
                original.graph.revision as i64,
                &edges,
            ))? != original.graph.digest
        {
            return Err(Error::Corrupt);
        }
        let retired: bool = row.get("retired");
        let outcome_bytes = self.get_slot(tx, wait, &request_digest, "outcome").await?;
        let completion_bytes = self
            .get_slot(tx, wait, &request_digest, "completion")
            .await?;
        let resume_bytes = self.get_slot(tx, wait, &request_digest, "resume").await?;
        match snapshot.outcome() {
            Some(outcome) => {
                if outcome_bytes.as_deref().map(|v| v.as_slice())
                    != Some(serde_json::to_vec(outcome)?.as_slice())
                    || row.get::<Option<String>, _>("terminal_digest").as_deref()
                        != Some(outcome.outcome.digest.as_str())
                {
                    return Err(Error::Corrupt);
                }
                let completion: Completion =
                    serde_json::from_slice(&completion_bytes.ok_or(Error::Corrupt)?)?;
                let resume: Option<engine::ResumeIntent> = resume_bytes
                    .as_ref()
                    .map(|b| serde_json::from_slice(b))
                    .transpose()?;
                if completion.wait_id != wait
                    || completion.outcome_digest != outcome.outcome.digest
                    || completion.resume != resume
                    || (!retired && resume != snapshot.resume_intent())
                {
                    return Err(Error::Corrupt);
                }
                if let Some(resume) = &resume
                    && (resume.intent_id != original.ids.resume_intent_id
                        || resume.outcome != outcome.outcome
                        || resume.continuation.continuation_id != original.ids.continuation_id
                        || resume.continuation.generation != reg.continuation_generation
                        || resume.continuation.wait != reg.wait_ref())
                {
                    return Err(Error::Corrupt);
                }
            }
            None => {
                if outcome_bytes.is_some() || completion_bytes.is_some() || resume_bytes.is_some() {
                    return Err(Error::Corrupt);
                }
            }
        }
        let cancel_bytes = self
            .get_slot(tx, wait, &request_digest, "cancellation")
            .await?;
        if retired {
            let cancel: Cancellation =
                serde_json::from_slice(&cancel_bytes.ok_or(Error::Corrupt)?)?;
            if cancel.wait_id != wait
                || cancel.anchor != original.anchor
                || Some(cancel.cancellation_id) != row.get::<Option<Uuid>, _>("cancel_operation_id")
                || snapshot.resume_intent().is_some()
            {
                return Err(Error::Corrupt);
            }
        } else if cancel_bytes.is_some() {
            return Err(Error::Corrupt);
        }
        let work =
            sqlx::query("SELECT lease_id,lease_expires_ms FROM wait_owner_work WHERE wait_id=$1")
                .bind(wait)
                .fetch_one(&mut **tx)
                .await?;
        Ok(Stored {
            observation: Observation {
                registration: receipt,
                snapshot,
                retired,
            },
            original,
            request_digest,
            lease: work.get("lease_id"),
            lease_expires: work
                .get::<Option<i64>, _>("lease_expires_ms")
                .map(|v| v as u64),
        })
    }
    pub(super) async fn save(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        stored: &Stored,
        next: &engine::Snapshot,
        current: &Current,
    ) -> Result<()> {
        Self::bounded(next)?;
        let wait = stored.observation.registration.wait_id;
        let old = &stored.observation.snapshot;
        if next.revision() != old.revision() + 1 {
            return Err(Error::Stale);
        }
        let retired = stored.observation.retired || current.cancellation.is_some();
        let terminal = next.outcome().map(|o| o.outcome.digest.as_str());
        sqlx::query("UPDATE wait_owner_records SET revision=$2,logical_now_ms=$3,next_due_ms=$4,phase=$5,dirty_generation=$6,work_generation=$7,retired=$8,cancel_operation_id=COALESCE(cancel_operation_id,$9),terminal_digest=$10,resume_state=$11 WHERE wait_id=$1 AND revision=$12")
            .bind(wait).bind(next.revision() as i64).bind(next.logical_now_ms() as i64).bind(next.next_due_ms() as i64).bind(phase(next)).bind(next.dirty_generation() as i64).bind(next.work_generation() as i64)
            .bind(retired).bind(current.cancellation).bind(terminal).bind(if retired{"retired"}else if next.outcome().is_some(){"pending"}else{"none"}).bind(old.revision() as i64).execute(&mut **tx).await?;
        self.put_slot(
            tx,
            wait,
            &stored.request_digest,
            "snapshot",
            next.revision(),
            &next.encode()?,
        )
        .await?;
        self.put_slot(
            tx,
            wait,
            &stored.request_digest,
            "work",
            next.revision(),
            &work_bytes(next)?,
        )
        .await?;
        if old.outcome().is_none()
            && let Some(outcome) = next.outcome()
        {
            self.put_slot(
                tx,
                wait,
                &stored.request_digest,
                "outcome",
                1,
                &serde_json::to_vec(outcome)?,
            )
            .await?;
            let resume = next.resume_intent();
            if let Some(resume) = &resume {
                self.put_slot(
                    tx,
                    wait,
                    &stored.request_digest,
                    "resume",
                    1,
                    &serde_json::to_vec(resume)?,
                )
                .await?;
            }
            self.put_slot(
                tx,
                wait,
                &stored.request_digest,
                "completion",
                1,
                &serde_json::to_vec(&Completion {
                    wait_id: wait,
                    outcome_digest: outcome.outcome.digest.clone(),
                    resume,
                })?,
            )
            .await?;
        }
        if !stored.observation.retired
            && let Some(cancellation_id) = current.cancellation
        {
            self.put_slot(
                tx,
                wait,
                &stored.request_digest,
                "cancellation",
                1,
                &serde_json::to_vec(&Cancellation {
                    wait_id: wait,
                    cancellation_id,
                    anchor: stored.original.anchor.clone(),
                })?,
            )
            .await?;
            sqlx::query("UPDATE wait_owner_tasks SET active_wait_id=NULL,generation=generation+1 WHERE workspace_id=$1 AND project_id=$2 AND task_id=$3 AND active_wait_id=$4")
                .bind(current.anchor.scope.workspace_id).bind(current.anchor.scope.project_id).bind(current.anchor.binding.task_id).bind(wait).execute(&mut **tx).await?;
        }
        if next.outcome().is_some() {
            sqlx::query("UPDATE wait_owner_edges SET active=FALSE WHERE wait_id=$1 AND active")
                .bind(wait)
                .execute(&mut **tx)
                .await?;
        }
        let keep_lease = stored.lease.is_some()
            && next.pending_work().map(|w| w.0) == old.pending_work().map(|w| w.0)
            && next.outcome().is_none()
            && next.phase() != &engine::Phase::RecoveryHeld;
        let state = if keep_lease {
            "leased"
        } else if next.outcome().is_some() {
            "done"
        } else if next.phase() == &engine::Phase::RecoveryHeld {
            "held"
        } else if next.pending_work().is_some() {
            "pending"
        } else {
            "idle"
        };
        sqlx::query("UPDATE wait_owner_work SET generation=$2,state=$3,next_due_ms=$4,lease_id=$6,lease_expires_ms=$7,transition_at_ms=CASE WHEN $6::uuid IS NOT NULL THEN transition_at_ms ELSE $5 END WHERE wait_id=$1")
            .bind(wait).bind(next.work_generation() as i64).bind(state).bind(next.next_due_ms() as i64).bind(next.logical_now_ms() as i64).bind(if keep_lease{stored.lease}else{None}).bind(if keep_lease{stored.lease_expires.map(|v|v as i64)}else{None}).execute(&mut **tx).await?;
        Ok(())
    }
    pub async fn observe(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        wait: Uuid,
    ) -> Result<Observation> {
        let (owner, original_attempt) = self.peek(wait).await?;
        if owner != node.id() || attempt != original_attempt {
            return Err(Error::NotFound);
        }
        let preflight = self.preflight(node, attempt, true).await?;
        let mut tx = self.transaction().await?;
        self.gates(&mut tx, &preflight.anchor, false).await?;
        let mut current = self.current(&mut tx, node, attempt, true).await?;
        self.mailbox_head(&mut tx, &current.anchor).await?;
        let stored = self.load(&mut tx, wait, true).await?;
        current.now = current
            .now
            .max(stored.observation.snapshot.logical_now_ms());
        current.refresh(&self.state)?;
        work::same_owner(&stored.original.anchor, &current.anchor)?;
        if !stored.observation.retired
            && (current.cancellation.is_some()
                || (stored.observation.snapshot.outcome().is_none()
                    && (current.now >= stored.observation.snapshot.deadline_ms()
                        || current.anchor.source_access != stored.original.anchor.source_access)))
        {
            let plan = stored.observation.snapshot.step(
                engine::Input::Drive,
                &current.fence,
                current.now,
            )?;
            self.save(&mut tx, &stored, &plan.next, &current).await?;
            tx.commit().await?;
            return Ok(Observation {
                registration: stored.observation.registration,
                snapshot: plan.next,
                retired: current.cancellation.is_some(),
            });
        }
        tx.commit().await?;
        Ok(stored.observation)
    }
    pub(super) async fn validate_retained(&self) -> Result<()> {
        // One consistent startup snapshot; decrypt only one bounded envelope at
        // a time. Keys are already loaded. There is no FS/network IO here.
        let mut tx = self.transaction().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;
        crate::store::wait38::validate(&mut tx)
            .await
            .map_err(|_| Error::Corrupt)?;
        let mut after = Uuid::nil();
        let mut seen = 0;
        loop {
            let wait: Option<Uuid> = sqlx::query_scalar(
                "SELECT wait_id FROM wait_owner_records WHERE wait_id>$1 ORDER BY wait_id LIMIT 1",
            )
            .bind(after)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(wait) = wait else { break };
            seen += 1;
            if seen > 2048 {
                return Err(Error::Capacity);
            }
            self.load(&mut tx, wait, false).await?;
            after = wait;
        }
        tx.commit().await?;
        Ok(())
    }
}
