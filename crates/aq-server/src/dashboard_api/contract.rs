//! dashboard-read-1 projections. These read claims never grant execution/control.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(transparent)]
pub struct Revision(#[schemars(regex(pattern = "^[1-9][0-9]{0,39}$"))] pub String);
/// Reference pins retain the owner's numeric revision or exact content digest.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ReferenceRevision {
    Revision(Revision),
    Digest(Digest),
}
impl From<Revision> for ReferenceRevision {
    fn from(value: Revision) -> Self {
        Self::Revision(value)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Timestamp(
    #[schemars(regex(
        pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\\.[0-9]{3}Z$"
    ))]
    pub String,
);
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Digest(#[schemars(regex(pattern = "^sha256:[0-9a-f]{64}$"))] pub String);
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    AllAuthorized,
    Project { project_id: Uuid },
}
impl Scope {
    pub fn project(&self) -> Option<Uuid> {
        match self {
            Self::AllAuthorized => None,
            Self::Project { project_id } => Some(*project_id),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum ApiVersion {
    #[serde(rename = "dashboard-read-1")]
    Read1,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub state: Completeness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Reported,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub observed_at: Timestamp,
    pub fresh_until: Timestamp,
    pub source: String,
    pub quality: Quality,
    pub coverage: Completeness,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T, V = ApiVersion> {
    pub api_version: V,
    pub request_id: Uuid,
    pub server_time: Timestamp,
    pub scope: Scope,
    pub data: T,
    pub observation: Observation,
    pub pending_operations: Vec<Uuid>,
    pub coverage: Coverage,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum CapabilityVersion {
    #[serde(rename = "1")]
    V1,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_retention: Option<super::actions::OperationRetention>,
    pub name: String,
    pub version: CapabilityVersion,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub id: Uuid,
    pub display_name: String,
    pub authorization_revision: Revision,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub version: String,
    pub dashboard_digest: Option<Digest>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContractRef {
    pub name: String,
    pub version: ContractVersion,
    pub digest: Digest,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum ContractVersion {
    #[serde(rename = "dashboard-read-1")]
    Read1,
    #[serde(rename = "dashboard-actions-1")]
    Actions1,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub page_size: u16,
    pub max_page_size: u16,
    pub snapshot_members: u16,
    pub snapshot_ttl_ms: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Supervisor {
    pub status: String,
    pub conversation_id: Option<Uuid>,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub principal: Principal,
    pub product: Product,
    pub contracts: Vec<ContractRef>,
    pub capabilities: Vec<Capability>,
    pub limits: Limits,
    pub default_scope: Scope,
    pub global_supervisor: Supervisor,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Accuracy {
    Exact,
    Estimated,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Total {
    pub value: Option<u32>,
    pub accuracy: Accuracy,
}
impl Total {
    pub fn unknown() -> Self {
        Self {
            value: None,
            accuracy: Accuracy::Unknown,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub snapshot_id: Uuid,
    pub total: Total,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectRef {
    pub id: Uuid,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<ReferenceRevision>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TitledRef {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<ReferenceRevision>,
    pub title: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionIdentity {
    Remote {
        node_id: Uuid,
        boot_id: Uuid,
        attempt_id: Uuid,
        session_id: Uuid,
        assignment_generation: Revision,
    },
    Central {
        authority_id: Uuid,
        session_id: Uuid,
        generation: Revision,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub harness: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessObservation {
    pub state: String,
    pub observed_at: Timestamp,
    pub fresh_until: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitRef {
    pub id: Uuid,
    pub explanation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub state: String,
    pub label: Option<String>,
}
/// Historical authenticated worker receipts, independent of process freshness.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionLifecycle {
    pub started_at: Option<Timestamp>,
    pub stopped_at: Option<Timestamp>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionSummary {
    pub id: Uuid,
    pub label: String,
    pub task: Option<TitledRef>,
    pub attempt_id: Option<Uuid>,
    pub project: ProjectRef,
    pub profile: Profile,
    pub authority: String,
    pub observation: ProcessObservation,
    pub wait: Option<WaitRef>,
    pub controller: Controller,
    pub terminal: Option<EntityRef>,
    pub session_identity: SessionIdentity,
    pub record_revision: Option<Revision>,
    pub execution_authority: String,
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<SessionLifecycle>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileRevision {
    pub name: String,
    pub revision: Revision,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub profile_revisions: Vec<ProfileRevision>,
    pub permissions: Vec<String>,
    pub last_activity_at: Option<Timestamp>,
    /// Independently pinned from the authorized retained offer, never usage input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_binding: Option<aq_lifecycle_api::Binding>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub running: Total,
    pub ready: Total,
    pub needs_you: Total,
    pub recovering: Total,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    pub kind: String,
    pub explanation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InMotion {
    pub project: ProjectRef,
    pub running: Total,
    pub next_constraint: Option<Constraint>,
    pub last_progress_at: Option<Timestamp>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overview {
    pub counts: Counts,
    pub attention: Vec<AttentionItem>,
    pub in_motion: Vec<InMotion>,
    pub recent_outcomes: Vec<RecentOutcome>,
    #[serde(default)]
    pub sources: Vec<SourceObservation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    Decision,
    Question,
    Blocked,
    Failed,
    UncertainOperation,
    WorkerOffline,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionDescriptor {
    pub kind: String,
    pub label: String,
    pub subject: EntityRef,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttentionItem {
    pub id: String,
    pub kind: AttentionKind,
    pub title: String,
    pub project: Option<ProjectRef>,
    pub reason: String,
    pub impact: String,
    pub since: Option<Timestamp>,
    pub subject: EntityRef,
    pub action: Option<ActionDescriptor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecentOutcome {
    pub subject: TitledRef,
    pub project: ProjectRef,
    pub outcome: String,
    pub at: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceObservation {
    pub source: String,
    pub project: Option<ProjectRef>,
    pub observation: Option<Observation>,
    pub coverage: Coverage,
    pub total: Total,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuotaState {
    Known,
    Unknown,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    pub starts_at: Option<Timestamp>,
    pub resets_at: Option<Timestamp>,
    #[schemars(range(min = 0.0, max = 100.0))]
    pub used_percent: Option<f64>,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: String,
    pub state: QuotaState,
    pub observation: Observation,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderScope {
    ProviderAccount,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsage {
    pub provider_id: String,
    pub account_ref: String,
    pub account_label: String,
    pub scope: ProviderScope,
    pub windows: Vec<QuotaWindow>,
    pub preferred_window_id: Option<String>,
    pub availability_ref: Option<EntityRef>,
}
/// X1/X2 installation capacity: slot load, per-node placement eligibility and
/// per-account provider availability. Owner-only and installation-wide.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    pub slots: SlotSummary,
    pub nodes: Vec<NodeCapacity>,
    pub providers: Vec<ProviderAvailability>,
}
/// Sandbox slots across online, unrevoked workers. `occupied` counts every
/// task-holding attempt; `total` and `idle` are unknown while any online
/// worker advertises no slot total (host-native workers).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotSummary {
    pub total: Total,
    pub occupied: Total,
    pub idle: Total,
    pub observed_at: Timestamp,
    pub coverage: Coverage,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerHealth {
    Online,
    Offline,
    Revoked,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityState {
    Eligible,
    Ineligible,
}
/// Why placement would refuse new work on a node. Ceiling reasons are the
/// placement refusal codes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IneligibleReason {
    WorkerOffline,
    WorkerRevoked,
    CapabilityUnreported,
    CapabilityExpired,
    NodeConcurrencyCeilingReached,
    NodeSpendCeilingReached,
    NodeSpendUnaccounted,
    NodeDiskPressure,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Eligibility {
    pub state: EligibilityState,
    pub reasons: Vec<IneligibleReason>,
}
/// A node's slots: `total`/`idle` are null when no backend advertises them.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeSlots {
    pub total: Option<u32>,
    pub occupied: u32,
    pub idle: Option<u32>,
}
/// The latest SBX25 storage report for a node.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeStorage {
    pub used_bps: u16,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_free_bytes: u64,
    pub disk_reserved_bytes: u64,
    pub preserved_count: u64,
    pub preserved_bytes: u64,
    pub observed_at: Timestamp,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeCapacity {
    pub node: EntityRef,
    pub name: String,
    pub health: WorkerHealth,
    pub last_seen_at: Option<Timestamp>,
    pub backend_kind: Option<String>,
    pub slots: NodeSlots,
    pub eligibility: Eligibility,
    pub capability_observed_at: Option<Timestamp>,
    pub capability_fresh_until: Option<Timestamp>,
    pub storage: Option<NodeStorage>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityState {
    Available,
    Unavailable,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityReason {
    AccountDisabled,
    NoFreshNodeReport,
    CredentialUnavailableOnNodes,
}
/// One configured provider account's availability from unexpired node
/// credential reports. `observed_at` is the newest report used; it is null
/// when no unexpired node reports the account's credential profiles.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderAvailability {
    pub provider_id: String,
    pub account_ref: String,
    pub state: AvailabilityState,
    pub reason: Option<AvailabilityReason>,
    pub available_nodes: u32,
    pub observed_at: Option<Timestamp>,
    pub fresh_until: Option<Timestamp>,
}
/// Owner-specific identity: playbook names are not UUIDs.
#[derive(
    Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    Task,
    Session,
    Result,
    Playbook,
    Knowledge,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRef {
    pub kind: SearchType,
    pub id: String,
    pub revision: ReferenceRevision,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    pub r#ref: SearchRef,
    pub title: String,
    pub excerpt: String,
    pub project: ProjectRef,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchGroup {
    pub r#type: SearchType,
    pub hits: Vec<SearchHit>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub groups: Vec<SearchGroup>,
    pub sources: Vec<SourceObservation>,
    pub snapshot_id: Uuid,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Response {
    Bootstrap(Envelope<Bootstrap>),
    Overview(Envelope<Overview>),
    Search(Envelope<Search>),
    ProviderUsage(Envelope<Vec<ProviderUsage>>),
    Capacity(Envelope<Capacity>),
    Sessions(Envelope<Page<SessionSummary>>),
    Session(Box<Envelope<SessionDetail>>),
    Projects(Envelope<Page<ProjectRef>>),
}
pub fn schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Response)).expect("static schema")
}
pub fn schema_bytes() -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(&schema()).expect("static schema encoding");
    bytes.push(b'\n');
    bytes
}
