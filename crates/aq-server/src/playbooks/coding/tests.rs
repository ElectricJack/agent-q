//! Fake Host7 peer only: synchronous answers in each command's exact contract
//! shape; no daemon, no database, no sleeps.
use super::*;
use crate::playbooks::review_domain::HOST7_CAPABILITIES;
use aq_playbook_host::{
    DomainHost, HostMode, MemoryStorage, Owner, Request,
    runner::{Authority, DispatchResult, Host, Intent, Storage},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const DEADLINE: Duration = Duration::from_secs(5);
pub(crate) type Log = Arc<Mutex<Vec<(String, Value)>>>;

/// Answers every Host7 command synchronously. `reviews` are the review
/// dispositions in order; `checks` the check outcomes in order (default pass).
pub(crate) struct Peer {
    pub log: Log,
    pub reviews: Vec<&'static str>,
    pub registry: Registry,
}
impl Peer {
    fn count(&self, name: &str) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| n == name)
            .count()
    }
}
fn subject(id: String) -> Value {
    json!({"id": id, "revision": 1})
}
impl Host for Peer {
    fn authorize(&self, _: &Authority, c: &CommandContract) -> bool {
        self.registry.commands.get(&c.name) == Some(c)
    }
    fn dispatch(&mut self, intent: &Intent) -> DispatchResult {
        let name = intent.command.name.clone();
        let n = self.count(&name) + 1;
        let input = intent.input.clone();
        self.log.lock().unwrap().push((name.clone(), input.clone()));
        let (outcome, output) = match name.as_str() {
            "work.ensure" if input["kind"] == "review" => {
                let i = self
                    .log
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(c, v)| c == "work.ensure" && v["kind"] == "review")
                    .count();
                let disposition = self.reviews.get(i - 1).copied().unwrap_or("accepted");
                (
                    "completed",
                    json!({"disposition": disposition,
                           "reference": format!("aq.review.decision/r{i}"),
                           "subject": input["subject"]}),
                )
            }
            "work.ensure" => (
                "completed",
                json!({"disposition": "accepted", "reference": format!("work:{n}"),
                       "subject": subject(format!("change-{n}"))}),
            ),
            "candidate.prepare" => (
                "prepared",
                json!({"reference": format!("candidate:{n}"),
                       "subject": subject(format!("candidate-{n}"))}),
            ),
            "check.request" => (
                "passed",
                json!({
                    "check": input["check"],
                    "disposition": "passed",
                    "receipt": {
                        "digest": hash_bytes(format!("receipt-{n}").as_bytes()),
                        "executor_version": "fake",
                        "exit": {"code": 0, "stop": "exited"},
                        "inputs_digest": hash_bytes(b"inputs"),
                        "result_artifacts": [],
                    },
                    "reference": format!("check-receipt:{n}"),
                    "source": input["source"],
                    "subject": input["subject"],
                }),
            ),
            "candidate.publish" => (
                "published",
                json!({"reference": format!("publication:{n}"),
                       "subject": input["destination"]}),
            ),
            _ => {
                return DispatchResult::Unknown {
                    reason: format!("unexpected {name}"),
                };
            }
        };
        DispatchResult::Completed {
            outcome: outcome.into(),
            output,
        }
    }
    fn reconcile(&mut self, intent: &Intent) -> DispatchResult {
        DispatchResult::Unknown {
            reason: format!("no reconcile for {}", intent.command.name),
        }
    }
}
impl DomainHost for Peer {
    fn set_mode(&mut self, _: HostMode) {}
}

pub(crate) fn authority() -> Authority {
    Authority {
        actor: "owner".into(),
        scope: "project".into(),
        capabilities: HOST7_CAPABILITIES.iter().map(|c| c.to_string()).collect(),
    }
}
pub(crate) fn checks() -> Vec<RequiredCheck> {
    vec![RequiredCheck {
        id: "cargo-test".into(),
        validator_id: "rust.cargo-test".into(),
        validator_version: "1".into(),
        validator_digest: hash_bytes(b"validator"),
    }]
}
pub(crate) fn request(key: &str) -> CodingRequest {
    CodingRequest {
        subject: json!({"id": "task-1", "revision": 1}),
        context: "Implement the thing".into(),
        base: json!({"id": "main", "revision": 1}),
        destination: json!({"id": "main", "revision": 1}),
        source: json!({"format": "sha1", "hex": "0".repeat(40)}),
        visibility: "project".into(),
        key: key.into(),
    }
}
pub(crate) async fn owner(
    namespace: &str,
    store: MemoryStorage,
    log: Log,
    reviews: Vec<&'static str>,
) -> Owner {
    let registry = CatalogProfile::Host7.registry();
    let peer_registry = registry.clone();
    Owner::start_registered(
        namespace.into(),
        aq_playbook_host::Limits::default(),
        registry,
        move || Ok(store),
        move |_| {
            Ok(Peer {
                log,
                reviews,
                registry: peer_registry,
            })
        },
    )
    .await
    .unwrap()
}
pub(crate) async fn install(owner: &Owner, artifacts: &CodingArtifacts) {
    for a in artifacts.install_order() {
        owner
            .call(
                Request::Install {
                    artifact: Box::new(a.clone()),
                },
                DEADLINE,
            )
            .await
            .unwrap();
    }
}
/// Ticks until every run is terminal; returns the root runs' outcomes.
pub(crate) async fn settle(
    owner: &Owner,
    store: &MemoryStorage,
    namespace: &str,
) -> Vec<(String, Option<String>)> {
    for now in 10..200 {
        owner
            .call(Request::Tick { now_ms: now }, DEADLINE)
            .await
            .unwrap();
        let state = store.clone().load(namespace).unwrap().state;
        if state.runs.values().all(|r| r.status.terminal()) {
            return state
                .runs
                .values()
                .filter(|r| r.parent.is_none())
                .map(|r| (r.artifact.clone(), r.outcome.clone()))
                .collect();
        }
    }
    panic!("coding runs did not settle");
}

#[tokio::test]
async fn coding_policy_review_rejection_creates_rework_attempt() {
    let policy = CodingPolicy::project_default(checks());
    let artifacts = policy.build().unwrap();
    let store = MemoryStorage::default();
    let log: Log = Arc::default();
    let owner = owner(
        "coding",
        store.clone(),
        log.clone(),
        vec!["changes_requested"],
    )
    .await;
    install(&owner, &artifacts).await;
    owner
        .call(
            Request::Activate {
                binding: ROOT.into(),
                artifact: artifacts.root.digest.clone(),
                authority: authority(),
            },
            DEADLINE,
        )
        .await
        .unwrap();
    owner
        .call(
            Request::Admit {
                binding: ROOT.into(),
                entry: "main".into(),
                input: policy.input(&request("task-1")),
                authority: authority(),
                now_ms: 1,
            },
            DEADLINE,
        )
        .await
        .unwrap();
    let roots = settle(&owner, &store, "coding").await;
    assert_eq!(
        roots,
        vec![(artifacts.root.digest.clone(), Some("published".into()))]
    );

    let log = log.lock().unwrap().clone();
    let work: Vec<_> = log
        .iter()
        .filter(|(c, _)| c == "work.ensure")
        .map(|(_, v)| v)
        .collect();
    let kinds: Vec<_> = work.iter().map(|v| v["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["implementation", "review", "repair", "review"]);
    // The rework attempt carries the rejecting review's evidence reference.
    assert_eq!(work[2]["context"], "aq.review.decision/r1");
    assert_eq!(work[2]["logical_key"], "task-1/a1/work");
    // Review is cross-family and keyed per attempt.
    assert!(
        work[1]["context"]
            .as_str()
            .unwrap()
            .contains("reviewer_family=openai")
    );
    assert_ne!(work[1]["logical_key"], work[3]["logical_key"]);
    // The repaired change is a new candidate: checks run again on it.
    let checks: Vec<_> = log
        .iter()
        .filter(|(c, _)| c == "check.request")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(checks.len(), 2);
    assert_ne!(checks[0]["subject"], checks[1]["subject"]);
    // Publish only once, with the check receipt and the review as evidence.
    let publish: Vec<_> = log
        .iter()
        .filter(|(c, _)| c == "candidate.publish")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(publish.len(), 1);
    let evidence: Vec<_> = publish[0]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(evidence, ["check-receipt:2", "aq.review.decision/r2"]);
    assert_eq!(publish[0]["candidate"], checks[1]["subject"]);
}

#[test]
fn same_family_review_is_refused() {
    let mut p = CodingPolicy::project_default(checks());
    p.reviewer_family = " Anthropic ".into();
    assert_eq!(p.build().unwrap_err(), CodingError::SameFamily);
}
