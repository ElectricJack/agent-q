//! C4 placement facts for a B1-registered sandbox node: the activation-time
//! template pin, the authenticated backend advertisement, node ceilings and
//! the backend placement preference (docker-local, then boat, then
//! cloudflare-containers unless the project policy overrides it). A host-native
//! node without a registration gets no placement facts and is unchanged.
//!
//! The preference hold is bounded and explained: a lower-preference node is
//! refused with `placement_preferred_backend_available` only while a
//! higher-preference eligible peer has an unexpired capability snapshot and
//! free capacity, so the hold ends no later than that snapshot's expiry.
use crate::AppState;
use crate::error::AppError;
use aq_admission_sandbox::sandbox::{BackendAdvertisement, BackendCapability, BackendKind, Money};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub const DEFAULT_PREFERENCE: [BackendKind; 3] = [
    BackendKind::DockerLocal,
    BackendKind::Boat,
    BackendKind::CloudflareContainers,
];
const MAX_PEERS: i64 = 256;

/// Optional ProjectPolicy override of placement preference and the backend
/// capabilities the project requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementPolicy {
    #[serde(default)]
    pub preference: Vec<BackendKind>,
    #[serde(default)]
    pub required_capabilities: Vec<BackendCapability>,
}
impl PlacementPolicy {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut seen = Vec::new();
        for kind in &self.preference {
            if seen.contains(kind) {
                return Err("duplicate placement preference backend");
            }
            seen.push(*kind);
        }
        if self.preference.len() > 3 || self.required_capabilities.len() > 4 {
            return Err("placement policy bounds");
        }
        Ok(())
    }
}

/// The concurrency ceiling, then the spend ceiling. A configured spend ceiling
/// with no comparable accounted spend refuses rather than assuming zero.
pub fn ceiling(
    advertisement: &BackendAdvertisement,
    active: u32,
    spend: Option<&Money>,
) -> Option<aq_selection::NodeCeiling> {
    use aq_selection::NodeCeiling;
    if active >= advertisement.max_concurrent_sandboxes {
        return Some(NodeCeiling::Concurrency);
    }
    let limit = advertisement.spend_ceiling.as_ref()?;
    match spend {
        Some(spent) if spent.currency == limit.currency => {
            (spent.micros >= limit.micros).then_some(NodeCeiling::Spend)
        }
        _ => Some(NodeCeiling::SpendUnaccounted),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Peer {
    pub kind: BackendKind,
    /// Online, under its ceilings, with the required capabilities and the
    /// resolved template.
    pub eligible: bool,
}
fn rank(preference: &[BackendKind], kind: BackendKind) -> usize {
    preference
        .iter()
        .position(|k| *k == kind)
        .unwrap_or(preference.len())
}
/// The best eligible peer ranked strictly above `own`; unlisted kinds rank last.
pub fn preferred_peer(
    own: BackendKind,
    preference: &[BackendKind],
    peers: &[Peer],
) -> Option<BackendKind> {
    let own_rank = rank(preference, own);
    peers
        .iter()
        .filter(|p| p.eligible && rank(preference, p.kind) < own_rank)
        .min_by_key(|p| rank(preference, p.kind))
        .map(|p| p.kind)
}

fn convert<T: serde::de::DeserializeOwned>(value: impl Serialize) -> Result<T, AppError> {
    serde_json::from_value(serde_json::to_value(value).map_err(|_| AppError::Internal)?)
        .map_err(|_| AppError::Internal)
}
fn db(_: sqlx::Error) -> AppError {
    AppError::Internal
}

fn policy(state: &AppState, project_id: Uuid) -> Option<PlacementPolicy> {
    state
        .config
        .profiled_execution
        .as_ref()?
        .projects
        .iter()
        .find(|p| p.project_id == project_id)?
        .placement
        .clone()
}

async fn active(pool: &PgPool, nodes: &[Uuid]) -> Result<Vec<(Uuid, i64)>, AppError> {
    sqlx::query_as(
        "SELECT node_id, count(*) FROM execution_attempts WHERE holds_task AND node_id = ANY($1) GROUP BY node_id",
    )
    .bind(nodes)
    .fetch_all(pool)
    .await
    .map_err(db)
}
fn count(active: &[(Uuid, i64)], node: Uuid) -> u32 {
    active
        .iter()
        .find(|(n, _)| *n == node)
        .map_or(0, |(_, c)| u32::try_from(*c).unwrap_or(u32::MAX))
}
/// The stored registration was validated when the node registered; placement
/// reads only its authenticated backend advertisement.
#[derive(Deserialize)]
struct Registered {
    backend: BackendAdvertisement,
}

/// Placement facts for selection on `node_id`, or None for a host-native node.
pub async fn facts(
    state: &AppState,
    node_id: Uuid,
    catalog_digest: &str,
    project_id: Uuid,
    environment: &str,
    now_ms: u64,
) -> Result<Option<aq_selection::Placement>, AppError> {
    facts_in(
        &state.store.pool,
        policy(state, project_id).as_ref(),
        node_id,
        catalog_digest,
        environment,
        now_ms,
    )
    .await
}

pub async fn facts_in(
    pool: &PgPool,
    policy: Option<&PlacementPolicy>,
    node_id: Uuid,
    catalog_digest: &str,
    environment: &str,
    now_ms: u64,
) -> Result<Option<aq_selection::Placement>, AppError> {
    let own: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT registration FROM delegated_node_boots WHERE node_id=$1")
            .bind(node_id)
            .fetch_optional(pool)
            .await
            .map_err(db)?;
    let Some(own) = own else {
        return Ok(None);
    };
    let own: Registered = serde_json::from_value(own).map_err(|_| AppError::Internal)?;
    let policy = policy.cloned().unwrap_or(PlacementPolicy {
        preference: Vec::new(),
        required_capabilities: Vec::new(),
    });
    let preference = if policy.preference.is_empty() {
        DEFAULT_PREFERENCE.to_vec()
    } else {
        policy.preference.clone()
    };
    let resolutions = crate::templates::load(pool, catalog_digest).await?;
    let resolved = resolutions
        .iter()
        .find(|r| r.environment == environment)
        .map(|r| r.template_digest.as_str().to_owned());

    let peers: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT b.node_id, b.registration FROM delegated_node_boots b \
         JOIN intelligence_node_snapshots s ON s.node_id=b.node_id \
         WHERE b.node_id<>$1 AND (s.snapshot->>'expires_at_ms')::bigint > $2 \
         ORDER BY b.node_id LIMIT $3",
    )
    .bind(node_id)
    .bind(i64::try_from(now_ms).unwrap_or(i64::MAX))
    .bind(MAX_PEERS)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    let mut nodes: Vec<Uuid> = peers.iter().map(|(n, _)| *n).collect();
    nodes.push(node_id);
    let active = active(pool, &nodes).await?;
    // Spend accounting is H2; until then a configured spend ceiling refuses.
    let spend: Option<&Money> = None;
    let eligible = |adv: &BackendAdvertisement, node: Uuid| {
        ceiling(adv, count(&active, node), spend).is_none()
            && policy
                .required_capabilities
                .iter()
                .all(|c| adv.capabilities.contains(c))
            && resolved
                .as_deref()
                .is_some_and(|d| adv.templates.iter().any(|t| t.digest.as_str() == d))
    };
    let peers: Vec<Peer> = peers
        .into_iter()
        .filter_map(|(node, reg)| {
            // A malformed peer registration is never a reason to hold.
            let reg: Registered = serde_json::from_value(reg).ok()?;
            Some(Peer {
                kind: reg.backend.kind,
                eligible: eligible(&reg.backend, node),
            })
        })
        .collect();
    let backend = &own.backend;
    Ok(Some(aq_selection::Placement {
        capabilities: backend
            .capabilities
            .iter()
            .map(convert)
            .collect::<Result<_, _>>()?,
        required_capabilities: policy
            .required_capabilities
            .iter()
            .map(convert)
            .collect::<Result<_, _>>()?,
        templates: backend
            .templates
            .iter()
            .map(|t| {
                Ok(aq_selection::AdvertisedTemplate {
                    digest: convert(&t.digest)?,
                    harnesses: t
                        .harnesses
                        .iter()
                        .map(|h| aq_selection::AdvertisedHarness {
                            harness: h.harness.clone(),
                            version: h.version.clone(),
                        })
                        .collect(),
                })
            })
            .collect::<Result<_, AppError>>()?,
        environment_templates: resolutions
            .iter()
            .map(|r| {
                Ok(aq_selection::EnvironmentTemplate {
                    environment: r.environment.clone(),
                    template: convert(&r.template_digest)?,
                })
            })
            .collect::<Result<_, AppError>>()?,
        ceiling: ceiling(backend, count(&active, node_id), spend),
        preferred_peer: preferred_peer(backend.kind, &preference, &peers)
            .map(convert)
            .transpose()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aq_selection::NodeCeiling;

    fn adv(max: u32, spend_ceiling: Option<(&str, u64)>) -> BackendAdvertisement {
        serde_json::from_value(serde_json::json!({
            "kind": "boat",
            "capabilities": ["vm-isolation"],
            "max_concurrent_sandboxes": max,
            "spend_ceiling": spend_ceiling.map(|(c, m)| serde_json::json!({"currency": c, "micros": m})),
            "templates": [],
        }))
        .unwrap()
    }
    fn money(currency: &str, micros: u64) -> Money {
        Money {
            currency: currency.into(),
            micros,
        }
    }

    #[test]
    fn placement_prefers_local_then_boat_then_cloudflare() {
        use BackendKind::*;
        let peer = |kind, eligible| Peer { kind, eligible };
        let all = [
            peer(DockerLocal, true),
            peer(Boat, true),
            peer(CloudflareContainers, true),
        ];
        // Default order: only the most preferred eligible backend gets offers.
        assert_eq!(preferred_peer(DockerLocal, &DEFAULT_PREFERENCE, &all), None);
        assert_eq!(
            preferred_peer(Boat, &DEFAULT_PREFERENCE, &all),
            Some(DockerLocal)
        );
        assert_eq!(
            preferred_peer(CloudflareContainers, &DEFAULT_PREFERENCE, &all),
            Some(DockerLocal)
        );
        // An ineligible (offline, full or unsupported) local node holds nothing.
        let no_local = [
            peer(DockerLocal, false),
            peer(Boat, true),
            peer(CloudflareContainers, true),
        ];
        assert_eq!(preferred_peer(Boat, &DEFAULT_PREFERENCE, &no_local), None);
        assert_eq!(
            preferred_peer(CloudflareContainers, &DEFAULT_PREFERENCE, &no_local),
            Some(Boat)
        );
        let only_cloud = [peer(DockerLocal, false), peer(Boat, false)];
        assert_eq!(
            preferred_peer(CloudflareContainers, &DEFAULT_PREFERENCE, &only_cloud),
            None
        );
        // ProjectPolicy override reverses the order; unlisted kinds rank last.
        let override_ = [CloudflareContainers, Boat];
        assert_eq!(preferred_peer(CloudflareContainers, &override_, &all), None);
        assert_eq!(
            preferred_peer(DockerLocal, &override_, &all),
            Some(CloudflareContainers)
        );
        assert_eq!(
            preferred_peer(Boat, &override_, &all),
            Some(CloudflareContainers)
        );
        // A peer of the same backend never holds.
        assert_eq!(
            preferred_peer(Boat, &DEFAULT_PREFERENCE, &[peer(Boat, true)]),
            None
        );
        // Policy validation.
        let p = |preference: Vec<BackendKind>| PlacementPolicy {
            preference,
            required_capabilities: vec![],
        };
        assert!(p(vec![Boat, DockerLocal]).validate().is_ok());
        assert!(p(vec![Boat, Boat]).validate().is_err());
        let policy: PlacementPolicy = serde_json::from_value(serde_json::json!({
            "preference": ["cloudflare-containers"],
            "required_capabilities": ["nested-docker"]
        }))
        .unwrap();
        assert_eq!(policy.preference, [CloudflareContainers]);
    }

    #[test]
    fn node_at_spend_ceiling_refuses_with_reason() {
        let usd = |m| money("USD", m);
        assert_eq!(ceiling(&adv(2, None), 1, None), None);
        assert_eq!(
            ceiling(&adv(2, None), 2, None),
            Some(NodeCeiling::Concurrency)
        );
        let limited = adv(2, Some(("USD", 1_000)));
        assert_eq!(ceiling(&limited, 0, Some(&usd(999))), None);
        assert_eq!(
            ceiling(&limited, 0, Some(&usd(1_000))),
            Some(NodeCeiling::Spend)
        );
        assert_eq!(NodeCeiling::Spend.reason(), "node_spend_ceiling_reached");
        // Unaccounted or incomparable spend never degrades to "under ceiling".
        assert_eq!(
            ceiling(&limited, 0, None),
            Some(NodeCeiling::SpendUnaccounted)
        );
        assert_eq!(
            ceiling(&limited, 0, Some(&money("EUR", 0))),
            Some(NodeCeiling::SpendUnaccounted)
        );
        // Concurrency is checked first.
        assert_eq!(
            ceiling(&limited, 2, Some(&usd(5_000))),
            Some(NodeCeiling::Concurrency)
        );
    }

    #[tokio::test]
    async fn sandbox_facts_pin_template_and_hold_for_preferred_peer() {
        let db = crate::test_support::TestDb::create().await;
        let pool = db.store().await.pool;
        let hex = |c: char| std::iter::repeat_n(c, 64).collect::<String>();
        let user = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users(id,email,role,created_at) VALUES($1,'o@example.test','owner',now())",
        )
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO profile_catalog_artifacts(digest,artifact,created_by,created_at_ms) VALUES($1,'\x7b\x7d',$2,1)")
            .bind(hex('f')).bind(user).execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        crate::templates::store(
            &mut tx,
            &hex('f'),
            &[crate::templates::Resolution {
                environment: "ci@1".into(),
                template_digest: serde_json::from_value(serde_json::json!(hex('a'))).unwrap(),
            }],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let node = |kind: &str, expires: u64| {
            let pool = pool.clone();
            let kind = kind.to_owned();
            let template = hex('a');
            async move {
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO nodes(id,name,credential_hash,worker_version,protocol_version,enrolled_at) VALUES($1,'n',$2,'t',1,now())")
                    .bind(id).bind(id.to_string()).execute(&pool).await.unwrap();
                let backend = serde_json::json!({"backend":{"kind":kind,"capabilities":["vm-isolation"],"max_concurrent_sandboxes":1,"spend_ceiling":null,
                    "templates":[{"digest":template,"harnesses":[{"harness":"codex","version":"1"}],"estimated_cold_start_ms":1,"estimated_cold_start_cost":{"currency":"USD","micros":0}}]}});
                sqlx::query("INSERT INTO delegated_node_boots(node_id,boot_id,registration) VALUES($1,$2,$3)")
                    .bind(id).bind(Uuid::new_v4()).bind(backend).execute(&pool).await.unwrap();
                sqlx::query("INSERT INTO intelligence_node_snapshots(node_id,boot_id,sequence,revision,snapshot) VALUES($1,$2,1,1,$3)")
                    .bind(id).bind(Uuid::new_v4()).bind(serde_json::json!({"expires_at_ms":expires})).execute(&pool).await.unwrap();
                id
            }
        };
        let cloud = node("cloudflare-containers", 2_000).await;
        let catalog = hex('f');
        let facts = |id, now| facts_in(&pool, None, id, &catalog, "ci@1", now);
        let only = facts(cloud, 1_000).await.unwrap().unwrap();
        assert_eq!(only.preferred_peer, None);
        assert_eq!(only.ceiling, None);
        assert_eq!(only.capabilities, ["vm-isolation"]);
        assert_eq!(only.environment_templates[0].template.as_str(), hex('a'));
        assert_eq!(only.templates[0].harnesses[0].version, "1");
        let boat = node("boat", 1_500).await;
        let held = facts(cloud, 1_000).await.unwrap().unwrap();
        assert_eq!(held.preferred_peer, Some(aq_selection::BackendKind::Boat));
        // The hold is bounded by the peer's snapshot expiry.
        assert_eq!(
            facts(cloud, 1_600).await.unwrap().unwrap().preferred_peer,
            None
        );
        // A required capability the peer lacks makes it ineligible; no hold.
        let nested = PlacementPolicy {
            preference: vec![],
            required_capabilities: vec![BackendCapability::NestedDocker],
        };
        let refused = facts_in(&pool, Some(&nested), cloud, &hex('f'), "ci@1", 1_000)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(refused.preferred_peer, None);
        assert_eq!(refused.required_capabilities, ["nested-docker"]);
        assert!(
            facts(boat, 1_000)
                .await
                .unwrap()
                .unwrap()
                .preferred_peer
                .is_none()
        );
        // Host-native nodes carry no placement facts.
        assert_eq!(facts(Uuid::new_v4(), 1_000).await.unwrap(), None);
    }
}
