//! D9 default coding policy, sealed from the pinned Host7 (catalog 2.0.0)
//! command contracts only: `work.ensure`, `check.request`,
//! `candidate.prepare`, `candidate.publish` and the `operation.get`
//! reconciliation command their asynchronous completions name.
//!
//! The policy is four artifacts. Bind names are unique and there is no
//! assignment, so the sync/async completion paths and the failure origins
//! converge through `Call` children whose outcome output is uniform:
//!
//! - `aq.coding.work`: one `work.ensure` (sync `completed`, or `accepted` then
//!   the `aq.operation.updated` completion) → `accepted` / `rejected` /
//!   `escalated`.
//! - `aq.coding.check`: one `check.request` (sync, or `accepted` then the
//!   `aq.check.receipted` receipt) → `passed` / `failed` / `escalated`.
//! - `aq.coding.attempt`: work → `candidate.prepare` → every required check on
//!   the prepared candidate → cross-family review (`work.ensure kind=review`) →
//!   `candidate.publish` with the check and review evidence → `published`, or
//!   `rework{reference}` naming the failing evidence.
//! - `aq.coding`: the implementation attempt, then up to `max_repairs` repair
//!   attempts, each handed the previous failing evidence reference as context.
//!
//! Evidence is bound to the prepared candidate subject, so a new candidate
//! (any subject change) runs every check and the review again. Review output
//! is published as review evidence next to, never instead of, the check
//! receipts; the host's publication guard verifies each in its owning store.
use super::review_domain::CatalogProfile;
use aq_playbook_host::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const ROOT: &str = "aq.coding";
pub const ATTEMPT: &str = "aq.coding.attempt";
pub const WORK: &str = "aq.coding.work";
pub const CHECK: &str = "aq.coding.check";
const COMPILER: &str = "aq-server D9 coding policy builder 1";
/// Bounded rework budget: the root is unrolled, one attempt step per budget.
pub const MAX_REPAIRS: usize = 8;
const WORK_WAIT_MS: u64 = 43_200_000;
const CHECK_WAIT_MS: u64 = 3_600_000;
const TEXT: usize = 4096;

/// One required check, pinned to an exact validator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredCheck {
    pub id: String,
    pub validator_id: String,
    pub validator_version: String,
    pub validator_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodingPolicy {
    pub implementer_family: String,
    pub reviewer_family: String,
    pub checks: Vec<RequiredCheck>,
    pub max_repairs: usize,
}

/// One coding run's request; `input` turns it into the root entry input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodingRequest {
    /// The work item (task) subject the implementation is ensured for.
    pub subject: Value,
    pub context: String,
    pub base: Value,
    pub destination: Value,
    /// Exact source the checks run on (`{format, hex}`).
    pub source: Value,
    pub visibility: String,
    /// Prefix of the per-attempt idempotent logical keys.
    pub key: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CodingError {
    #[error("implementer and reviewer must be different model families")]
    SameFamily,
    #[error("coding policy invalid: {0}")]
    Invalid(String),
}

/// The four sealed artifacts, in install order (children first).
#[derive(Clone, Debug)]
pub struct CodingArtifacts {
    pub work: Artifact,
    pub check: Artifact,
    pub attempt: Artifact,
    pub root: Artifact,
}
impl CodingArtifacts {
    pub fn install_order(&self) -> [&Artifact; 4] {
        [&self.work, &self.check, &self.attempt, &self.root]
    }
}

impl CodingPolicy {
    /// The project default: Claude implements, a different family reviews.
    pub fn project_default(checks: Vec<RequiredCheck>) -> Self {
        Self {
            implementer_family: "anthropic".into(),
            reviewer_family: "openai".into(),
            checks,
            max_repairs: 2,
        }
    }

    pub fn build(&self) -> Result<CodingArtifacts, CodingError> {
        let family = |f: &str| f.trim().to_ascii_lowercase();
        if family(&self.implementer_family).is_empty() || family(&self.reviewer_family).is_empty() {
            return Err(CodingError::Invalid("model family required".into()));
        }
        if family(&self.implementer_family) == family(&self.reviewer_family) {
            return Err(CodingError::SameFamily);
        }
        if self.max_repairs > MAX_REPAIRS || self.checks.len() > 32 {
            return Err(CodingError::Invalid("budget exceeds bound".into()));
        }
        let ids: BTreeSet<_> = self.checks.iter().map(|c| &c.id).collect();
        if ids.len() != self.checks.len() {
            return Err(CodingError::Invalid("duplicate check id".into()));
        }
        let b = Builder::new();
        let work = b.work()?;
        let check = b.check()?;
        let attempt = b.attempt(self, &work, &check)?;
        let root = b.root(self, &attempt)?;
        Ok(CodingArtifacts {
            work,
            check,
            attempt,
            root,
        })
    }

    /// Root entry input for one coding run.
    pub fn input(&self, r: &CodingRequest) -> Value {
        let keys: serde_json::Map<String, Value> = (0..=self.max_repairs)
            .map(|i| {
                (
                    format!("a{i}"),
                    json!({"work": format!("{}/a{i}/work", r.key),
                           "review": format!("{}/a{i}/review", r.key)}),
                )
            })
            .collect();
        json!({"subject": r.subject, "context": r.context, "base": r.base,
               "destination": r.destination, "source": r.source,
               "visibility": r.visibility, "keys": keys})
    }

    fn review_context(&self) -> String {
        format!(
            "Cross-family review of the prepared candidate. reviewer_family={}; \
             implementer_family={}. Approve only a correct change; a review never \
             substitutes for a required check or a human decision.",
            self.reviewer_family.trim(),
            self.implementer_family.trim()
        )
    }
}

fn lit(v: Value) -> Expr {
    Expr::literal(v)
}
fn r(root: &str, path: &[&str]) -> Expr {
    Expr::Ref {
        root: root.into(),
        path: path.iter().map(|s| (*s).into()).collect(),
    }
}
fn fields(items: &[(&str, Expr)]) -> Expr {
    Expr::Object {
        fields: items.iter().cloned().map(|(k, v)| (k.into(), v)).collect(),
    }
}
fn eq(left: Expr, right: &str) -> Expr {
    Expr::Eq {
        left: Box::new(left),
        right: Box::new(lit(json!(right))),
    }
}
fn edges(op: Operation, edges: &[(&str, &str)]) -> Step {
    Step {
        op,
        edges: edges
            .iter()
            .map(|(a, b)| ((*a).into(), (*b).into()))
            .collect(),
        on_error: Some("failure".into()),
    }
}
fn decide(predicate: Expr, yes: &str, no: &str) -> Step {
    edges(
        Operation::Decide { predicate },
        &[("true", yes), ("false", no), ("unknown", "failure")],
    )
}
fn ret(outcome: &str, output: Expr) -> Step {
    Step {
        op: Operation::Return {
            outcome: outcome.into(),
            output,
        },
        edges: BTreeMap::new(),
        on_error: None,
    }
}
/// `{reference, subject}` projected from a bound result.
fn result_of(root: &str, prefix: &[&str]) -> Expr {
    let at = |leaf: &str| {
        let mut p = prefix.to_vec();
        p.push(leaf);
        r(root, &p)
    };
    fields(&[("reference", at("reference")), ("subject", at("subject"))])
}
fn call(dependency: &str, input: Expr, bind: &str, outcomes: &[(&str, &str)]) -> Step {
    edges(
        Operation::Call {
            dependency: dependency.into(),
            entry: "main".into(),
            input,
            mode: CallMode::Wait,
            bind: bind.into(),
        },
        outcomes,
    )
}

struct Builder {
    registry: Registry,
}
impl Builder {
    fn new() -> Self {
        Self {
            registry: CatalogProfile::Host7.registry(),
        }
    }
    fn command(&self, name: &str) -> Result<&CommandContract, CodingError> {
        self.registry
            .commands
            .get(name)
            .ok_or_else(|| CodingError::Invalid(format!("Host7 lacks {name}")))
    }
    fn input_field(&self, command: &str, field: &str) -> Result<Schema, CodingError> {
        match &self.command(command)?.input {
            Schema::Object { properties, .. } => properties
                .get(field)
                .cloned()
                .ok_or_else(|| CodingError::Invalid(format!("{command} lacks {field}"))),
            _ => Err(CodingError::Invalid(format!("{command} input"))),
        }
    }
    fn subject(&self) -> Result<Schema, CodingError> {
        self.input_field("work.ensure", "subject")
    }
    /// `{reference, subject}`: the uniform child success output.
    fn result(&self) -> Result<Schema, CodingError> {
        Ok(Schema::object(BTreeMap::from([
            ("reference".into(), Schema::string(TEXT)),
            ("subject".into(), self.subject()?),
        ])))
    }
    fn reference_only() -> Schema {
        Schema::object(BTreeMap::from([("reference".into(), Schema::string(TEXT))]))
    }
    /// Escalation is an empty object so a Call bind stays an object union.
    fn escalated() -> Schema {
        Schema::object(BTreeMap::new())
    }
    fn invoke(
        &self,
        name: &str,
        input: Expr,
        bind: &str,
        success: &[(&str, &str)],
    ) -> Result<Step, CodingError> {
        let c = self.command(name)?;
        let fingerprint = c
            .fingerprint()
            .map_err(|e| CodingError::Invalid(e.to_string()))?;
        let mut s = edges(
            Operation::Invoke {
                command: ContractRef {
                    name: name.into(),
                    fingerprint,
                },
                input,
                bind: bind.into(),
                retry: None,
            },
            &[],
        );
        for outcome in c.outcomes.keys() {
            s.edges.insert(outcome.clone(), "failure".into());
        }
        for (a, b) in success {
            s.edges.insert((*a).into(), (*b).into());
        }
        Ok(s)
    }
    fn event(&self, name: &str) -> Result<ContractRef, CodingError> {
        let e = self
            .registry
            .events
            .get(name)
            .ok_or_else(|| CodingError::Invalid(format!("Host7 lacks {name}")))?;
        Ok(ContractRef {
            name: name.into(),
            fingerprint: e
                .fingerprint()
                .map_err(|e| CodingError::Invalid(e.to_string()))?,
        })
    }
    fn wait(
        &self,
        event: &str,
        handle: &str,
        ms: u64,
        bind: &str,
        next: &str,
    ) -> Result<Step, CodingError> {
        Ok(edges(
            Operation::Wait {
                event: Some(self.event(event)?),
                correlation: BTreeMap::from([("operation".into(), r(handle, &["handle"]))]),
                minimum_revision: Some(lit(json!(0))),
                deadline_ms: ms,
                bind: bind.into(),
            },
            &[("received", next), ("timeout", "failure")],
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn seal(
        &self,
        id: &str,
        purpose: &str,
        input: Schema,
        outcomes: BTreeMap<String, Schema>,
        start: &str,
        mut steps: BTreeMap<String, Step>,
        children: &[&Artifact],
        escalated: Expr,
    ) -> Result<Artifact, CodingError> {
        let mut commands = BTreeSet::new();
        let mut events = BTreeSet::new();
        for step in steps.values() {
            match &step.op {
                Operation::Invoke { command, .. } => {
                    commands.insert(command.name.clone());
                }
                Operation::Wait { event: Some(e), .. } => {
                    events.insert(e.name.clone());
                }
                _ => {}
            }
        }
        for name in commands.clone() {
            if let Some(a) = &self.command(&name)?.asynchronous {
                commands.insert(a.reconciliation_command.clone());
                events.insert(a.completion_event.clone());
            }
        }
        let mut registry = self.registry.clone();
        registry.commands.retain(|n, _| commands.contains(n));
        registry.events.retain(|n, _| events.contains(n));
        steps.insert("failure".into(), ret("escalated", escalated));
        let required_capabilities = registry
            .commands
            .values()
            .flat_map(|c| c.capabilities.clone())
            .chain(
                children
                    .iter()
                    .flat_map(|c| c.content.playbook.required_capabilities.clone()),
            )
            .collect();
        let dependencies = children
            .iter()
            .map(|a| {
                let binding = a.content.playbook.id.clone();
                (
                    binding.clone(),
                    CallableDependency {
                        binding,
                        selection: DependencySelection::Exact,
                        artifact: a.digest.clone(),
                        interfaces: a.content.playbook.entries.clone(),
                        capabilities: a.content.playbook.required_capabilities.clone(),
                        scope: "project".into(),
                        dependencies: a
                            .content
                            .dependencies
                            .values()
                            .flat_map(|d| {
                                std::iter::once(d.artifact.clone()).chain(d.dependencies.clone())
                            })
                            .collect(),
                    },
                )
            })
            .collect();
        let source = format!("{COMPILER}\n{id}\n{purpose}\n");
        let a = Artifact::seal(ArtifactContent {
            format_version: 1,
            compiler: COMPILER.into(),
            source: SourceBinding {
                digest: hash_bytes(source.as_bytes()),
                header: SourceHeader {
                    format_version: 1,
                    id: id.into(),
                    scope: "project".into(),
                    interfaces: BTreeSet::from(["main".into()]),
                    definitions: BTreeSet::new(),
                },
            },
            playbook: Playbook {
                id: id.into(),
                purpose: purpose.into(),
                scope: "project".into(),
                entries: BTreeMap::from([(
                    "main".into(),
                    EntryPoint {
                        input,
                        outcomes,
                        start: start.into(),
                    },
                )]),
                steps,
                triggers: vec![],
                limits: Limits::default(),
                required_capabilities,
            },
            registry,
            dependencies,
            definitions: BTreeMap::new(),
            source_map: BTreeMap::new(),
        })
        .map_err(|e| CodingError::Invalid(e.to_string()))?;
        a.verify()
            .map_err(|e| CodingError::Invalid(format!("{id}: {e}")))?;
        Ok(a)
    }

    fn work(&self) -> Result<Artifact, CodingError> {
        let outcomes = BTreeMap::from([
            ("accepted".into(), self.result()?),
            ("rejected".into(), self.result()?),
            ("escalated".into(), Self::escalated()),
        ]);
        let steps = BTreeMap::from([
            (
                "request".into(),
                self.invoke(
                    "work.ensure",
                    r("input", &[]),
                    "work",
                    &[("completed", "assess"), ("accepted", "await")],
                )?,
            ),
            (
                "assess".into(),
                decide(
                    eq(r("work", &["disposition"]), "accepted"),
                    "accepted",
                    "rejected",
                ),
            ),
            ("accepted".into(), ret("accepted", result_of("work", &[]))),
            ("rejected".into(), ret("rejected", result_of("work", &[]))),
            (
                "await".into(),
                self.wait(
                    "aq.operation.updated",
                    "work",
                    WORK_WAIT_MS,
                    "completion",
                    "completed",
                )?,
            ),
            (
                "completed".into(),
                decide(
                    eq(r("completion", &["outcome"]), "completed"),
                    "assess_event",
                    "failure",
                ),
            ),
            (
                "assess_event".into(),
                decide(
                    eq(r("completion", &["output", "disposition"]), "accepted"),
                    "accepted_event",
                    "rejected_event",
                ),
            ),
            (
                "accepted_event".into(),
                ret("accepted", result_of("completion", &["output"])),
            ),
            (
                "rejected_event".into(),
                ret("rejected", result_of("completion", &["output"])),
            ),
        ]);
        self.seal(
            WORK,
            "Ensure one work item and converge its sync or async completion",
            self.command("work.ensure")?.input.clone(),
            outcomes,
            "request",
            steps,
            &[],
            fields(&[]),
        )
    }

    fn check(&self) -> Result<Artifact, CodingError> {
        let outcomes = BTreeMap::from([
            ("passed".into(), self.result()?),
            ("failed".into(), self.result()?),
            ("escalated".into(), Self::escalated()),
        ]);
        let steps = BTreeMap::from([
            (
                "request".into(),
                self.invoke(
                    "check.request",
                    r("input", &[]),
                    "check",
                    &[
                        ("passed", "passed"),
                        ("failed", "failed"),
                        ("accepted", "await"),
                    ],
                )?,
            ),
            ("passed".into(), ret("passed", result_of("check", &[]))),
            ("failed".into(), ret("failed", result_of("check", &[]))),
            (
                "await".into(),
                self.wait(
                    "aq.check.receipted",
                    "check",
                    CHECK_WAIT_MS,
                    "receipt",
                    "assess",
                )?,
            ),
            (
                "assess".into(),
                decide(
                    eq(r("receipt", &["outcome"]), "passed"),
                    "passed_event",
                    "assess_failed",
                ),
            ),
            (
                "assess_failed".into(),
                decide(
                    eq(r("receipt", &["outcome"]), "failed"),
                    "failed_event",
                    "failure",
                ),
            ),
            (
                "passed_event".into(),
                ret("passed", result_of("receipt", &["output"])),
            ),
            (
                "failed_event".into(),
                ret("failed", result_of("receipt", &["output"])),
            ),
        ]);
        self.seal(
            CHECK,
            "Request one required check and converge its sync or async receipt",
            self.command("check.request")?.input.clone(),
            outcomes,
            "request",
            steps,
            &[],
            fields(&[]),
        )
    }

    fn attempt_input(&self) -> Result<Schema, CodingError> {
        Ok(Schema::object(BTreeMap::from([
            ("work".into(), self.command("work.ensure")?.input.clone()),
            ("base".into(), self.subject()?),
            ("destination".into(), self.subject()?),
            (
                "source".into(),
                self.input_field("check.request", "source")?,
            ),
            ("review_key".into(), Schema::string(TEXT)),
            ("visibility".into(), Schema::string(TEXT)),
        ])))
    }

    fn attempt(
        &self,
        policy: &CodingPolicy,
        work: &Artifact,
        check: &Artifact,
    ) -> Result<Artifact, CodingError> {
        let rework = |bind: &str| ret("rework", fields(&[("reference", r(bind, &["reference"]))]));
        let mut steps = BTreeMap::from([
            (
                "work".into(),
                call(
                    WORK,
                    r("input", &["work"]),
                    "work",
                    &[
                        ("accepted", "prepare"),
                        ("rejected", "rework_work"),
                        ("escalated", "failure"),
                    ],
                ),
            ),
            ("rework_work".into(), rework("work")),
        ]);
        let first = if policy.checks.is_empty() {
            "review".to_string()
        } else {
            "check_0".to_string()
        };
        steps.insert(
            "prepare".into(),
            self.invoke(
                "candidate.prepare",
                fields(&[
                    ("base", r("input", &["base"])),
                    (
                        "sources",
                        Expr::Array {
                            items: vec![r("work", &["subject"])],
                        },
                    ),
                ]),
                "candidate",
                &[("prepared", &first), ("conflict", "rework_prepare")],
            )?,
        );
        steps.insert("rework_prepare".into(), rework("candidate"));
        let candidate_revision = || r("candidate", &["subject", "revision"]);
        let mut evidence = vec![];
        for (i, c) in policy.checks.iter().enumerate() {
            let bind = format!("check_{i}");
            let next = if i + 1 == policy.checks.len() {
                "review".to_string()
            } else {
                format!("check_{}", i + 1)
            };
            let failed = format!("rework_check_{i}");
            steps.insert(
                bind.clone(),
                call(
                    CHECK,
                    fields(&[
                        (
                            "check",
                            lit(json!({"id": c.id, "validator": {"id": c.validator_id,
                                "version": c.validator_version, "digest": c.validator_digest}})),
                        ),
                        ("source", r("input", &["source"])),
                        ("subject", r("candidate", &["subject"])),
                    ]),
                    &bind,
                    &[
                        ("passed", &next),
                        ("failed", &failed),
                        ("escalated", "failure"),
                    ],
                ),
            );
            steps.insert(failed, rework(&bind));
            evidence.push(fields(&[
                ("id", r(&bind, &["reference"])),
                ("revision", candidate_revision()),
            ]));
        }
        steps.insert(
            "review".into(),
            call(
                WORK,
                fields(&[
                    ("kind", lit(json!("review"))),
                    ("subject", r("candidate", &["subject"])),
                    ("context", lit(json!(policy.review_context()))),
                    ("lifetime", lit(json!("owned"))),
                    ("logical_key", r("input", &["review_key"])),
                    ("visibility", r("input", &["visibility"])),
                ]),
                "review",
                &[
                    ("accepted", "publish"),
                    ("rejected", "rework_review"),
                    ("escalated", "failure"),
                ],
            ),
        );
        steps.insert("rework_review".into(), rework("review"));
        evidence.push(fields(&[
            ("id", r("review", &["reference"])),
            ("revision", candidate_revision()),
        ]));
        steps.insert(
            "publish".into(),
            self.invoke(
                "candidate.publish",
                fields(&[
                    ("candidate", r("candidate", &["subject"])),
                    ("destination", r("input", &["destination"])),
                    ("evidence", Expr::Array { items: evidence }),
                ]),
                "publication",
                &[("published", "published"), ("conflict", "rework_publish")],
            )?,
        );
        steps.insert(
            "published".into(),
            ret("published", result_of("publication", &[])),
        );
        steps.insert("rework_publish".into(), rework("publication"));
        let outcomes = BTreeMap::from([
            ("published".into(), self.result()?),
            ("rework".into(), Self::reference_only()),
            ("escalated".into(), Self::escalated()),
        ]);
        self.seal(
            ATTEMPT,
            "One coding attempt: work, prepare, checks, cross-family review, publish",
            self.attempt_input()?,
            outcomes,
            "work",
            steps,
            &[work, check],
            fields(&[]),
        )
    }

    fn root(&self, policy: &CodingPolicy, attempt: &Artifact) -> Result<Artifact, CodingError> {
        let key = Schema::object(BTreeMap::from([
            ("work".into(), Schema::string(TEXT)),
            ("review".into(), Schema::string(TEXT)),
        ]));
        let keys = (0..=policy.max_repairs)
            .map(|i| (format!("a{i}"), key.clone()))
            .collect();
        let input = Schema::object(BTreeMap::from([
            ("subject".into(), self.subject()?),
            ("context".into(), Schema::string(TEXT)),
            ("base".into(), self.subject()?),
            ("destination".into(), self.subject()?),
            (
                "source".into(),
                self.input_field("check.request", "source")?,
            ),
            ("visibility".into(), Schema::string(TEXT)),
            ("keys".into(), Schema::object(keys)),
        ]));
        let mut steps = BTreeMap::new();
        for i in 0..=policy.max_repairs {
            let bind = format!("attempt_{i}");
            let a = format!("a{i}");
            let (kind, context) = if i == 0 {
                ("implementation", r("input", &["context"]))
            } else {
                // Repair carries the previous attempt's failing evidence.
                ("repair", r(&format!("attempt_{}", i - 1), &["reference"]))
            };
            let next = if i == policy.max_repairs {
                "exhausted".to_string()
            } else {
                format!("attempt_{}", i + 1)
            };
            let published = format!("published_{i}");
            steps.insert(
                bind.clone(),
                call(
                    ATTEMPT,
                    fields(&[
                        (
                            "work",
                            fields(&[
                                ("kind", lit(json!(kind))),
                                ("subject", r("input", &["subject"])),
                                ("context", context),
                                ("lifetime", lit(json!("owned"))),
                                ("logical_key", r("input", &["keys", &a, "work"])),
                                ("visibility", r("input", &["visibility"])),
                            ]),
                        ),
                        ("base", r("input", &["base"])),
                        ("destination", r("input", &["destination"])),
                        ("source", r("input", &["source"])),
                        ("review_key", r("input", &["keys", &a, "review"])),
                        ("visibility", r("input", &["visibility"])),
                    ]),
                    &bind,
                    &[
                        ("published", &published),
                        ("rework", &next),
                        ("escalated", "failure"),
                    ],
                ),
            );
            steps.insert(published, ret("published", result_of(&bind, &[])));
        }
        let last = format!("attempt_{}", policy.max_repairs);
        steps.insert(
            "exhausted".into(),
            ret(
                "exhausted",
                fields(&[("reference", r(&last, &["reference"]))]),
            ),
        );
        let outcomes = BTreeMap::from([
            ("published".into(), self.result()?),
            ("exhausted".into(), Self::reference_only()),
            ("escalated".into(), Schema::Null),
        ]);
        self.seal(
            ROOT,
            "Default coding policy: implement, check, cross-family review, publish, repair",
            input,
            outcomes,
            "attempt_0",
            steps,
            &[attempt],
            lit(Value::Null),
        )
    }
}

#[cfg(test)]
pub(crate) mod tests;
