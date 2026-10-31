//! AQ-owned transactions. Engine storage remains independent; all mailbox
//! effects and operation events share this transaction and its current grants.
use super::domain::*;
use crate::{
    Clock,
    agent_access::messaging,
    protected_storage::{Keyring, Sealed},
};
use aq_playbook_host::{
    model::{ContractRef, EventEnvelope, canonical_bytes},
    runner::{CancellationResult, Intent},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;
const SOURCE: &str = "48f19b80fc0734c403ef04e7181d570b9fe675597f35caf08f0b94571ca8cc70";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    pub namespace_id: Uuid,
    pub generation: i64,
    pub name: String,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub owner_user_id: Uuid,
}
impl Namespace {
    fn validate(&self) -> Result<()> {
        if [
            self.namespace_id,
            self.workspace_id,
            self.project_id,
            self.owner_user_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.generation <= 0
            || self.name.is_empty()
            || self.name.len() > 128
            || self.name.contains('\0')
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn engine_namespace(&self) -> String {
        format!("aq/{}/{}", self.namespace_id, self.generation)
    }
}
/// Process-local current-owner context, not a deserializable fresh HTTP permit.
#[derive(Clone, Debug)]
pub struct RuntimeOwner {
    namespace_id: Uuid,
    generation: i64,
    incarnation: Uuid,
    revision: i64,
}
#[derive(Clone)]
pub struct Service {
    pub namespace: Namespace,
    pool: PgPool,
    keys: Arc<Keyring>,
    clock: Clock,
}
#[derive(Clone, Debug)]
pub enum MessageResult {
    Queued(Value),
    Cancelled,
}
impl Service {
    pub async fn new(
        state: &crate::AppState,
        namespace: Namespace,
        keys: Arc<Keyring>,
    ) -> Result<Self> {
        namespace.validate()?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_millis(500))
            .connect_with(
                state
                    .store
                    .pool
                    .connect_options()
                    .as_ref()
                    .clone()
                    .application_name("aq-playbook-bridge"),
            )
            .await?;
        Ok(Self {
            namespace,
            pool,
            keys,
            clock: state.clock.clone(),
        })
    }
    fn now(&self) -> Result<i64> {
        let at = (self.clock)().timestamp_millis();
        if at < 0 {
            return Err(Error::Invalid);
        }
        Ok(at)
    }
    async fn tx(&self) -> Result<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('statement_timeout','2000ms',true),set_config('lock_timeout','500ms',true)")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
    async fn owner(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR SHARE")
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?;
        if role.as_deref() != Some("owner") {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
    async fn namespace_locked(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<sqlx::postgres::PgRow> {
        let row = sqlx::query(
            "SELECT * FROM playbook_namespaces WHERE namespace_id=$1 AND generation=$2 FOR UPDATE",
        )
        .bind(self.namespace.namespace_id)
        .bind(self.namespace.generation)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::Forbidden)?;
        if row.get::<Uuid, _>("owner_user_id") != self.namespace.owner_user_id
            || row.get::<Uuid, _>("workspace_id") != self.namespace.workspace_id
            || row.get::<Uuid, _>("project_id") != self.namespace.project_id
            || row.get::<String, _>("name") != self.namespace.name
            || row.get::<String, _>("catalog_digest") != digest(&registry())?
            || row.get::<String, _>("source_digest") != SOURCE
        {
            return Err(Error::Conflict);
        }
        Ok(row)
    }
    async fn budget(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        admissions: i64,
        effects: i64,
        cancellations: i64,
        outbox: i64,
        bytes: i64,
    ) -> Result<()> {
        if [admissions, effects, cancellations, outbox, bytes]
            .iter()
            .any(|v| *v < 0)
        {
            return Err(Error::Invalid);
        }
        let changed=sqlx::query("UPDATE playbook_namespaces SET admission_count=admission_count+$3,effect_count=effect_count+$4,cancellation_count=cancellation_count+$5,outbox_count=outbox_count+$6,retained_bytes=retained_bytes+$7 WHERE namespace_id=$1 AND generation=$2 AND admission_count+$3<=4096 AND effect_count+$4<=8192 AND cancellation_count+$5<=8192 AND outbox_count+$6<=16384 AND retained_bytes+$7<=67108864").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(admissions).bind(effects).bind(cancellations).bind(outbox).bind(bytes).execute(&mut **tx).await?.rows_affected();
        if changed != 1 {
            return Err(Error::Overloaded);
        }
        Ok(())
    }
    pub async fn initialize(&self, actor: Uuid) -> Result<()> {
        if actor != self.namespace.owner_user_id {
            return Err(Error::Forbidden);
        }
        let mut tx = self.tx().await?;
        Self::owner(&mut tx, actor).await?;
        let binding: Option<Value> = sqlx::query_scalar(
            "SELECT binding FROM project_execution_bindings WHERE project_id=$1 FOR SHARE",
        )
        .bind(self.namespace.project_id)
        .fetch_optional(&mut *tx)
        .await?;
        if !binding.is_some_and(|b| {
            b["enabled"] == true && b["graph_workspace_id"] == json!(self.namespace.workspace_id)
        }) {
            return Err(Error::Forbidden);
        }
        sqlx::query("INSERT INTO playbook_namespaces(namespace_id,generation,name,workspace_id,project_id,owner_user_id,catalog_digest,source_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(namespace_id) DO NOTHING").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&self.namespace.name).bind(self.namespace.workspace_id).bind(self.namespace.project_id).bind(actor).bind(digest(&registry())?).bind(SOURCE).execute(&mut *tx).await?;
        self.namespace_locked(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn runtime_revision(&self) -> Result<i64> {
        let mut tx = self.tx().await?;
        let row = self.namespace_locked(&mut tx).await?;
        let r = row.get("runtime_revision");
        tx.commit().await?;
        Ok(r)
    }
    /// Expected revision is captured before engine ownership acquisition. A delayed
    /// initializer may only replay this exact incarnation/revision, never refresh it.
    pub async fn adopt_runtime(&self, incarnation: Uuid, expected: i64) -> Result<RuntimeOwner> {
        if incarnation.is_nil() || !(0..i64::MAX - 1).contains(&expected) {
            return Err(Error::Invalid);
        }
        let mut tx = self.tx().await?;
        let row = self.namespace_locked(&mut tx).await?;
        Self::owner(&mut tx, self.namespace.owner_user_id).await?;
        let revision: i64 = row.get("runtime_revision");
        let old: Option<Uuid> = row.get("runtime_incarnation");
        if !(revision == expected || (revision == expected + 1 && old == Some(incarnation))) {
            return Err(Error::Fenced);
        }
        if revision == expected {
            sqlx::query("UPDATE playbook_namespaces SET runtime_revision=$2,runtime_incarnation=$3 WHERE namespace_id=$1").bind(self.namespace.namespace_id).bind(expected+1).bind(incarnation).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(RuntimeOwner {
            namespace_id: self.namespace.namespace_id,
            generation: self.namespace.generation,
            incarnation,
            revision: expected + 1,
        })
    }
    fn runtime(&self, row: &sqlx::postgres::PgRow, owner: &RuntimeOwner) -> Result<()> {
        if owner.namespace_id != self.namespace.namespace_id
            || owner.generation != self.namespace.generation
            || row.get::<Option<Uuid>, _>("runtime_incarnation") != Some(owner.incarnation)
            || row.get::<i64, _>("runtime_revision") != owner.revision
        {
            return Err(Error::Fenced);
        }
        Ok(())
    }
    pub async fn grant(
        &self,
        actor: Uuid,
        id: Uuid,
        scope: GrantScope,
        expires: i64,
    ) -> Result<()> {
        scope.validate()?;
        if actor != self.namespace.owner_user_id
            || id.is_nil()
            || scope.project_id != self.namespace.project_id
            || scope.workspace_id != self.namespace.workspace_id
        {
            return Err(Error::Forbidden);
        }
        let payload = digest(&(
            self.namespace.namespace_id,
            self.namespace.generation,
            id,
            &scope,
            expires,
            actor,
        ))?;
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        Self::owner(&mut tx, actor).await?;
        if let Some(old) = sqlx::query_scalar::<_, String>(
            "SELECT payload_digest FROM playbook_grants WHERE grant_id=$1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        {
            if old != payload {
                return Err(Error::Conflict);
            }
            tx.commit().await?;
            return Ok(());
        }
        let now = self.now()?;
        if expires <= now
            || expires > now.saturating_add(86_400_000)
            || ns.get::<i64, _>("grant_count") >= 1024
        {
            return Err(Error::Invalid);
        }
        let bytes = canonical_bytes(&scope).map_err(|_| Error::Invalid)?.len() as i64 + 1024;
        self.budget(&mut tx, 0, 0, 0, 0, bytes).await?;
        sqlx::query("INSERT INTO playbook_grants(grant_id,namespace_id,generation,revision,owner_user_id,scope,expires_at_ms,payload_digest) VALUES($1,$2,$3,1,$4,$5,$6,$7)").bind(id).bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(actor).bind(json!(scope)).bind(expires).bind(payload).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE playbook_namespaces SET grant_count=grant_count+1 WHERE namespace_id=$1",
        )
        .bind(self.namespace.namespace_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn revoke(&self, actor: Uuid, id: Uuid) -> Result<()> {
        if actor != self.namespace.owner_user_id {
            return Err(Error::Forbidden);
        }
        let mut tx = self.tx().await?;
        self.namespace_locked(&mut tx).await?;
        Self::owner(&mut tx, actor).await?;
        let changed=sqlx::query("UPDATE playbook_grants SET revoked_at_ms=COALESCE(revoked_at_ms,$4) WHERE grant_id=$1 AND namespace_id=$2 AND generation=$3").bind(id).bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(self.now()?).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(Error::Forbidden);
        }
        tx.commit().await?;
        Ok(())
    }
    async fn grant_locked(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        id: Uuid,
        revision: i64,
        live: bool,
    ) -> Result<GrantScope> {
        let row=sqlx::query("SELECT * FROM playbook_grants WHERE grant_id=$1 AND namespace_id=$2 AND generation=$3 FOR UPDATE").bind(id).bind(self.namespace.namespace_id).bind(self.namespace.generation).fetch_optional(&mut **tx).await?.ok_or(Error::Forbidden)?;
        if row.get::<i64, _>("revision") != revision
            || row.get::<Uuid, _>("owner_user_id") != self.namespace.owner_user_id
        {
            return Err(Error::Forbidden);
        }
        Self::owner(tx, self.namespace.owner_user_id).await?;
        if live
            && (row.get::<Option<i64>, _>("revoked_at_ms").is_some()
                || row.get::<i64, _>("expires_at_ms") <= self.now()?)
        {
            return Err(Error::Forbidden);
        }
        let scope: GrantScope = serde_json::from_value(row.get("scope"))?;
        scope.validate()?;
        Ok(scope)
    }
    fn identity(&self, intent: &Intent) -> Result<String> {
        digest(&(
            self.namespace.namespace_id,
            self.namespace.generation,
            &intent.id,
            &intent.run,
            &intent.step,
            intent.invocation,
            intent.deadline_at_ms,
            &intent.command,
            &intent.input_digest,
            &intent.authority,
        ))
    }
    async fn check_expiry(&self, tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
        let expiry: i64 =
            sqlx::query_scalar("SELECT expires_at_ms FROM playbook_grants WHERE grant_id=$1")
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
        if expiry <= self.now()? {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
    pub async fn authorize(&self, owner: &RuntimeOwner, id: Uuid, revision: i64) -> Result<()> {
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        self.grant_locked(&mut tx, id, revision, true).await?;
        self.check_expiry(&mut tx, id).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn send_message(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<MessageResult> {
        let input = validate_intent(intent, id, revision)?;
        let identity = self.identity(intent)?;
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        // Historical replay is exact but does not create or refresh any write authority.
        if let Some(row)=sqlx::query("SELECT request_digest,status,receipt FROM playbook_effects WHERE namespace_id=$1 AND generation=$2 AND intent_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).fetch_optional(&mut *tx).await? {
   self.grant_locked(&mut tx,id,revision,false).await?;if row.get::<String,_>("request_digest")!=identity{return Err(Error::Conflict)}let result=if row.get::<String,_>("status")=="queued"{MessageResult::Queued(row.get("receipt"))}else{MessageResult::Cancelled};tx.commit().await?;return Ok(result)
  }
        let scope = self.grant_locked(&mut tx, id, revision, true).await?;
        if !scope.permits(&input.recipient) || intent.deadline_at_ms <= self.now()? {
            return Err(Error::Forbidden);
        }
        if let Some(cancel)=sqlx::query_scalar::<_,String>("SELECT request_digest FROM playbook_cancellations WHERE namespace_id=$1 AND generation=$2 AND intent_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).fetch_optional(&mut *tx).await? {if cancel!=identity{return Err(Error::Conflict)}return Ok(MessageResult::Cancelled)}
        let op = stable_id(
            "aq.playbook.message.operation",
            self.namespace.namespace_id,
            self.namespace.generation,
            &intent.id,
        )?;
        let message = stable_id(
            "aq.playbook.message.identity",
            self.namespace.namespace_id,
            self.namespace.generation,
            &intent.id,
        )?;
        let recipient: messaging::Recipient = serde_json::from_value(json!(input.recipient))?;
        let response = messaging::owner_send_in_tx(
            &mut tx,
            messaging::Send {
                operation_id: op,
                message_id: message,
                thread_id: input.thread_id,
                recipient,
                body: input.body,
            },
            self.namespace.owner_user_id,
            self.now()? as u64,
        )
        .await
        .map_err(|e| match e {
            messaging::OwnerSendError::Invalid => Error::Invalid,
            messaging::OwnerSendError::NotFound => Error::Forbidden,
            messaging::OwnerSendError::Conflict => Error::Conflict,
            messaging::OwnerSendError::Unknown => Error::Unknown,
        })?;
        message_contract().outcomes["queued"]
            .validate(&response)
            .map_err(|_| Error::Unknown)?;
        let event_id = stable_id(
            "aq.playbook.message.queued.event",
            self.namespace.namespace_id,
            self.namespace.generation,
            &intent.id,
        )?;
        let c = operation_event();
        let at = self.now()?;
        let event = EventEnvelope {
            producer: "aq.host.bridge@1".into(),
            event_id: event_id.to_string(),
            contract: ContractRef {
                name: c.name.clone(),
                fingerprint: c.fingerprint().map_err(|_| Error::Invalid)?,
            },
            scope: "project".into(),
            correlation: BTreeMap::from([("operation".into(), op.to_string())]),
            subject: Some(input.recipient.scope().2.to_string()),
            revision: None,
            causation: Some(intent.id.clone()),
            occurred_at_ms: Some(at),
            observed_at_ms: Some(at),
            payload: json!({"operation_id":op,"message_id":message,"disposition":"queued"}),
        };
        let event_digest = digest(&event)?;
        let bytes = canonical_bytes(&event).map_err(|_| Error::Invalid)?;
        let aad = canonical_bytes(&(
            "aq.playbooks.outbox",
            1,
            self.namespace.namespace_id,
            self.namespace.generation,
            event_id,
            &event_digest,
        ))
        .map_err(|_| Error::Invalid)?;
        let sealed = self.keys.seal(&aad, &bytes).map_err(|_| Error::Unknown)?;
        let retained = canonical_bytes(&response)
            .map_err(|_| Error::Invalid)?
            .len() as i64
            + sealed.ciphertext.len() as i64
            + 4096;
        self.budget(&mut tx, 0, 1, 0, 1, retained).await?;
        sqlx::query("INSERT INTO playbook_effects(namespace_id,generation,intent_id,run_id,command_name,request_digest,command_fingerprint,input_digest,grant_id,grant_revision,operation_id,status,receipt,created_at_ms) VALUES($1,$2,$3,$4,'message.send',$5,$6,$7,$8,$9,$10,'queued',$11,$12)").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).bind(&intent.run).bind(&identity).bind(intent.command.fingerprint().map_err(|_|Error::Invalid)?).bind(&intent.input_digest).bind(id).bind(revision).bind(op).bind(&response).bind(at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO playbook_outbox(namespace_id,generation,producer,epoch,event_id,event_digest,sealed_version,key_id,nonce,ciphertext,created_at_ms,status) VALUES($1,$2,'aq.host.bridge@1',0,$3,$4,$5,$6,$7,$8,$9,'pending')").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(event_id).bind(event_digest).bind(sealed.version as i32).bind(sealed.key_id).bind(sealed.nonce).bind(sealed.ciphertext).bind(at).execute(&mut *tx).await?;
        self.check_expiry(&mut tx, id).await?;
        if intent.deadline_at_ms <= self.now()? {
            return Err(Error::Forbidden);
        }
        tx.commit().await?;
        Ok(MessageResult::Queued(response))
    }
    pub async fn reconcile(
        &self,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<Option<MessageResult>> {
        validate_intent(intent, id, revision)?;
        let identity = self.identity(intent)?;
        let mut tx = self.tx().await?;
        self.namespace_locked(&mut tx).await?;
        self.grant_locked(&mut tx, id, revision, false).await?;
        let row=sqlx::query("SELECT request_digest,status,receipt FROM playbook_effects WHERE namespace_id=$1 AND generation=$2 AND intent_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).fetch_optional(&mut *tx).await?;
        let result = match row {
            Some(row) => {
                if row.get::<String, _>("request_digest") != identity {
                    return Err(Error::Conflict);
                }
                Some(if row.get::<String, _>("status") == "queued" {
                    MessageResult::Queued(row.get("receipt"))
                } else {
                    MessageResult::Cancelled
                })
            }
            None => {
                let cancel=sqlx::query("SELECT request_digest,disposition FROM playbook_cancellations WHERE namespace_id=$1 AND generation=$2 AND intent_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).fetch_optional(&mut *tx).await?;
                if let Some(cancel) = cancel {
                    if cancel.get::<String, _>("request_digest") != identity {
                        return Err(Error::Conflict);
                    }
                    if cancel.get::<String, _>("disposition") == "confirmed" {
                        Some(MessageResult::Cancelled)
                    } else {
                        return Err(Error::Unknown);
                    }
                } else {
                    None
                }
            }
        };
        tx.commit().await?;
        Ok(result)
    }
    pub async fn cancel(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<CancellationResult> {
        let input = validate_intent(intent, id, revision)?;
        let identity = self.identity(intent)?;
        let key = aq_playbook_host::runner::cancellation_key(intent).map_err(|_| Error::Invalid)?;
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        let scope = self.grant_locked(&mut tx, id, revision, false).await?;
        if !scope.permits(&input.recipient) {
            return Err(Error::Forbidden);
        }
        if let Some(row)=sqlx::query("SELECT request_digest,disposition FROM playbook_cancellations WHERE namespace_id=$1 AND generation=$2 AND cancellation_key=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&key).fetch_optional(&mut *tx).await?{if row.get::<String,_>("request_digest")!=identity{return Err(Error::Conflict)}return Ok(if row.get::<String,_>("disposition")=="confirmed"{CancellationResult::Confirmed}else{CancellationResult::TooLate})}
        let existing=sqlx::query_scalar::<_,String>("SELECT request_digest FROM playbook_effects WHERE namespace_id=$1 AND generation=$2 AND intent_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(&intent.id).fetch_optional(&mut *tx).await?;
        if existing.as_ref().is_some_and(|d| d != &identity) {
            return Err(Error::Conflict);
        }
        let disposition = if existing.is_some() {
            "too_late"
        } else {
            "confirmed"
        };
        self.budget(&mut tx, 0, 0, 1, 0, 2048).await?;
        sqlx::query("INSERT INTO playbook_cancellations(namespace_id,generation,cancellation_key,intent_id,request_digest,disposition,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(key).bind(&intent.id).bind(identity).bind(disposition).bind(self.now()?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(if existing.is_some() {
            CancellationResult::TooLate
        } else {
            CancellationResult::Confirmed
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRequest {
    pub request_id: Uuid,
    pub grant_id: Uuid,
    pub grant_revision: i64,
    pub binding: String,
    pub expected_activation_revision: u64,
    pub entry: String,
    pub input: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdmissionStatus {
    pub request_id: Uuid,
    pub engine_key: String,
    pub status: String,
    pub receipt: Option<aq_playbook_host::runner::KeyedAdmissionReceipt>,
}
/// Durable recovery data, not fresh write authorization. Only the driver can
/// inspect the retained exact request, and it must separately check current grant.
pub struct RetainedAdmission {
    pub status: AdmissionStatus,
    pub(super) request: aq_playbook_host::runner::KeyedAdmissionRequest,
    pub(super) grant_id: Uuid,
    pub(super) grant_revision: i64,
}
impl Service {
    fn aad(&self, domain: &str, id: Uuid, digest: &str) -> Result<Vec<u8>> {
        canonical_bytes(&(
            domain,
            1,
            self.namespace.namespace_id,
            self.namespace.generation,
            id,
            digest,
        ))
        .map_err(|_| Error::Invalid)
    }
    pub async fn prepare(&self, actor: Uuid, request: PrepareRequest) -> Result<AdmissionStatus> {
        if actor != self.namespace.owner_user_id
            || request.request_id.is_nil()
            || request.grant_id.is_nil()
            || request.grant_revision <= 0
            || request.expected_activation_revision == 0
            || request.expected_activation_revision > i64::MAX as u64
        {
            return Err(Error::Invalid);
        }
        if request.binding.is_empty()
            || request.binding.len() > 512
            || request.binding.contains('\0')
            || request.entry.is_empty()
            || request.entry.len() > 512
            || request.entry.contains('\0')
        {
            return Err(Error::Invalid);
        }
        let key = stable_id(
            "aq.playbook.manual.admission",
            self.namespace.namespace_id,
            self.namespace.generation,
            &request.request_id.to_string(),
        )?
        .to_string();
        let engine_request = aq_playbook_host::runner::KeyedAdmissionRequest {
            key: key.clone(),
            binding: request.binding,
            expected_activation_revision: request.expected_activation_revision,
            entry: request.entry,
            input: request.input,
            authority: authority(request.grant_id, request.grant_revision),
        };
        let bytes = canonical_bytes(&engine_request).map_err(|_| Error::Invalid)?;
        if bytes.len() > 65536 {
            return Err(Error::Invalid);
        }
        let digest = digest(&engine_request)?;
        let mut tx = self.tx().await?;
        self.namespace_locked(&mut tx).await?;
        self.grant_locked(&mut tx, request.grant_id, request.grant_revision, false)
            .await?;
        if let Some(old)=sqlx::query("SELECT request_digest,status,receipt FROM playbook_admissions WHERE namespace_id=$1 AND generation=$2 AND request_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(request.request_id).fetch_optional(&mut *tx).await? {if old.get::<String,_>("request_digest")!=digest{return Err(Error::Conflict)}let receipt:Option<Value>=old.get("receipt");let status=AdmissionStatus{request_id:request.request_id,engine_key:key,status:old.get("status"),receipt:receipt.map(serde_json::from_value).transpose()?};tx.commit().await?;return Ok(status)}
        self.grant_locked(&mut tx, request.grant_id, request.grant_revision, true)
            .await?;
        let sealed = self
            .keys
            .seal(
                &self.aad("aq.playbooks.admission", request.request_id, &digest)?,
                &bytes,
            )
            .map_err(|_| Error::Unknown)?;
        self.budget(&mut tx, 1, 0, 0, 0, sealed.ciphertext.len() as i64 + 4096)
            .await?;
        sqlx::query("INSERT INTO playbook_admissions(namespace_id,generation,request_id,request_digest,engine_key,grant_id,grant_revision,expected_activation_revision,sealed_version,key_id,nonce,ciphertext,created_at_ms,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'pending')").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(request.request_id).bind(digest).bind(&key).bind(request.grant_id).bind(request.grant_revision).bind(request.expected_activation_revision as i64).bind(sealed.version as i32).bind(sealed.key_id).bind(sealed.nonce).bind(sealed.ciphertext).bind(self.now()?).execute(&mut *tx).await?;
        self.check_expiry(&mut tx, request.grant_id).await?;
        tx.commit().await?;
        Ok(AdmissionStatus {
            request_id: request.request_id,
            engine_key: key,
            status: "pending".into(),
            receipt: None,
        })
    }
    pub async fn admission(&self, actor: Uuid, id: Uuid) -> Result<RetainedAdmission> {
        if actor != self.namespace.owner_user_id || id.is_nil() {
            return Err(Error::Forbidden);
        }
        let mut tx = self.tx().await?;
        self.namespace_locked(&mut tx).await?;
        Self::owner(&mut tx, actor).await?;
        let row=sqlx::query("SELECT * FROM playbook_admissions WHERE namespace_id=$1 AND generation=$2 AND request_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error::Forbidden)?;
        let digest: String = row.get("request_digest");
        let sealed = Sealed {
            version: row.get::<i32, _>("sealed_version") as u32,
            key_id: row.get("key_id"),
            nonce: row.get("nonce"),
            ciphertext: row.get("ciphertext"),
        };
        let bytes = self
            .keys
            .open(&self.aad("aq.playbooks.admission", id, &digest)?, &sealed)
            .map_err(|_| Error::Unknown)?;
        let request: aq_playbook_host::runner::KeyedAdmissionRequest =
            serde_json::from_slice(&bytes)?;
        let key: String = row.get("engine_key");
        let grant_id: Uuid = row.get("grant_id");
        let revision: i64 = row.get("grant_revision");
        if super::domain::digest(&request)? != digest
            || request.key != key
            || request.authority != authority(grant_id, revision)
        {
            return Err(Error::Conflict);
        }
        let receipt: Option<Value> = row.get("receipt");
        let status = AdmissionStatus {
            request_id: id,
            engine_key: key,
            status: row.get("status"),
            receipt: receipt.map(serde_json::from_value).transpose()?,
        };
        tx.commit().await?;
        Ok(RetainedAdmission {
            status,
            request,
            grant_id,
            grant_revision: revision,
        })
    }
    pub async fn acknowledge_admission(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        receipt: aq_playbook_host::runner::KeyedAdmissionReceipt,
    ) -> Result<AdmissionStatus> {
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        Self::owner(&mut tx, self.namespace.owner_user_id).await?;
        let row=sqlx::query("SELECT request_digest,engine_key,expected_activation_revision,receipt,status FROM playbook_admissions WHERE namespace_id=$1 AND generation=$2 AND request_id=$3 FOR UPDATE").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error::Forbidden)?;
        if receipt.request_digest != row.get::<String, _>("request_digest")
            || receipt.key != row.get::<String, _>("engine_key")
            || receipt.activation_revision
                != row.get::<i64, _>("expected_activation_revision") as u64
        {
            return Err(Error::Conflict);
        }
        if let Some(old) = row.get::<Option<Value>, _>("receipt")
            && old != json!(receipt)
        {
            return Err(Error::Conflict);
        }
        sqlx::query("UPDATE playbook_admissions SET status='engine_accepted',receipt=$4 WHERE namespace_id=$1 AND generation=$2 AND request_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).bind(json!(receipt)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(AdmissionStatus {
            request_id: id,
            engine_key: receipt.key.clone(),
            status: "engine_accepted".into(),
            receipt: Some(receipt),
        })
    }
    pub async fn pending_admissions(&self, owner: &RuntimeOwner, limit: u32) -> Result<Vec<Uuid>> {
        if !(1..=16).contains(&limit) {
            return Err(Error::Invalid);
        }
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        Self::owner(&mut tx, self.namespace.owner_user_id).await?;
        let rows=sqlx::query_scalar("SELECT request_id FROM playbook_admissions WHERE namespace_id=$1 AND generation=$2 AND status='pending' ORDER BY created_at_ms,request_id LIMIT $3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(limit as i64).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn outbox(
        &self,
        owner: &RuntimeOwner,
        after: i64,
        limit: u32,
    ) -> Result<Vec<(i64, Uuid, EventEnvelope)>> {
        if after < 0 || !(1..=16).contains(&limit) {
            return Err(Error::Invalid);
        }
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        Self::owner(&mut tx, self.namespace.owner_user_id).await?;
        let rows=sqlx::query("SELECT * FROM playbook_outbox WHERE namespace_id=$1 AND generation=$2 AND sequence>$3 AND (status!='engine_accepted' OR NOT fanout_complete) ORDER BY sequence LIMIT $4").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(after).bind(limit as i64).fetch_all(&mut *tx).await?;
        let mut result = Vec::new();
        for row in rows {
            let id: Uuid = row.get("event_id");
            let digest: String = row.get("event_digest");
            let sealed = Sealed {
                version: row.get::<i32, _>("sealed_version") as u32,
                key_id: row.get("key_id"),
                nonce: row.get("nonce"),
                ciphertext: row.get("ciphertext"),
            };
            let bytes = self
                .keys
                .open(&self.aad("aq.playbooks.outbox", id, &digest)?, &sealed)
                .map_err(|_| Error::Unknown)?;
            let event: EventEnvelope = serde_json::from_slice(&bytes)?;
            if super::domain::digest(&event)? != digest
                || event.event_id != id.to_string()
                || event.producer != "aq.host.bridge@1"
            {
                return Err(Error::Conflict);
            }
            aq_playbook_host::model::validate_event(&event, &registry())
                .map_err(|_| Error::Invalid)?;
            result.push((row.get("sequence"), id, event));
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn acknowledge_event(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        receipt: aq_playbook_host::runner::EventDisposition,
        fanout_complete: bool,
    ) -> Result<()> {
        let expected = digest(&("aq.host.bridge@1", 0_u64, id.to_string()))?;
        if receipt.key != expected || receipt.destinations > 10000 {
            return Err(Error::Conflict);
        }
        let stable = json!({"key":receipt.key,"destinations":receipt.destinations});
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        let old:Option<Option<Value>>=sqlx::query_scalar("SELECT engine_receipt FROM playbook_outbox WHERE namespace_id=$1 AND generation=$2 AND event_id=$3 FOR UPDATE").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).fetch_optional(&mut *tx).await?;
        let old = old.ok_or(Error::Forbidden)?;
        if old.is_some_and(|v| v != stable) {
            return Err(Error::Conflict);
        }
        sqlx::query("UPDATE playbook_outbox SET status='engine_accepted',engine_receipt=$4,fanout_complete=fanout_complete OR $5 WHERE namespace_id=$1 AND generation=$2 AND event_id=$3").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).bind(stable).bind(fanout_complete).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn retire_runtime(&self, owner: &RuntimeOwner) -> Result<()> {
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        let next = owner.revision.checked_add(1).ok_or(Error::Overloaded)?;
        if ns.get::<Option<Uuid>, _>("runtime_incarnation").is_none()
            && ns.get::<i64, _>("runtime_revision") == next
        {
            tx.commit().await?;
            return Ok(());
        }
        self.runtime(&ns, owner)?;
        Self::owner(&mut tx, self.namespace.owner_user_id).await?;
        sqlx::query("UPDATE playbook_namespaces SET runtime_revision=$2,runtime_incarnation=NULL WHERE namespace_id=$1").bind(self.namespace.namespace_id).bind(next).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Service {
    pub async fn authorize_recovery(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
    ) -> Result<()> {
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        self.grant_locked(&mut tx, id, revision, false).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn hold_admission(&self, owner: &RuntimeOwner, id: Uuid) -> Result<()> {
        let mut tx = self.tx().await?;
        let ns = self.namespace_locked(&mut tx).await?;
        self.runtime(&ns, owner)?;
        sqlx::query("UPDATE playbook_admissions SET status='unknown' WHERE namespace_id=$1 AND generation=$2 AND request_id=$3 AND receipt IS NULL AND status='pending'").bind(self.namespace.namespace_id).bind(self.namespace.generation).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Service {
    pub(super) fn current_time(&self) -> i64 {
        (self.clock)().timestamp_millis()
    }
}
