# F1a Central Session Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist the independent central session lifecycle while recording the physical supervisor host as blocked on `crisp-nexus-12`.

**Architecture:** A dormant PostgreSQL owner in `aq-central-authority` serializes each actor through a locked head row. It validates retained reducer states, commits permit transitions before returning them, retains every generation, and derives successor mail fences from that history. A required trusted verifier checks plan authorization and physical/mail evidence; no production implementation, route, daemon activation or physical authority claim is introduced before the subject-neutral host contract exists.

**Tech Stack:** Rust 1.96.1, sqlx 0.8, PostgreSQL, existing AQ-17 reducer.

**Spec:** Task `steady-harbor-87.1`, `docs/implementation/AQ-17-central-authority-core.md:47-55`, and operator instruction `msg-4ced906430644c58b79ae1ca9352aff3`.

## Global Constraints

- Each repository builds alone with pinned dependencies and a committed Cargo.lock.
- No placeholder task IDs or weakening of the frozen LaunchManifest validator.
- Stop proof and complete retained mail history precede successors.
- No provider credential reads, process launch, worker enrollment or Docker access in the owner.
- Existing v1 wire contracts and historical migrations remain unchanged.
- No architecture/spec document changes in this task.
- Run focused checks through the AQ resource gate; unavailable checks are not passes.
- The overall task remains blocked until the real host acceptance tests and cross-family review are complete.

## Task 1: Durable lifecycle and successor history

**Files:**
- Create `crates/aq-central-authority/src/owner.rs` and `src/owner/history.rs`.
- Modify `crates/aq-central-authority/{Cargo.toml,src/lib.rs,README.md}` and `Cargo.lock`.
- Create `crates/aq-server/migrations/0045_central_sessions.sql`.
- Register migration and bump state format in `crates/aq-server/src/{store.rs,lib.rs}` and `Dockerfile`.
- Create `crates/aq-server/tests/central_sessions.rs`.
- Create `docs/implementation/F1a-evidence/test_cargo.py` for resource-gated commands.

**Interfaces:**
- Consumes `Run::{new,transition,successor}`, `CentralPlan`, `Clock`, `PermitData`, and verified host/mail artifact references.
- Produces `SessionOwner<V>` with `prepare`, `start`, `renew`, `report`, `inspect`, and `generation`.
- `EvidenceVerifier` authorizes plans and verifies evidence within the caller's trusted process. The owner takes no arbitrary HTTP caller input and supplies no permissive verifier.
- `Snapshot` contains revision and validated Run; `Receipt` contains Snapshot and reducer Decision. A refused transition may still persist expiry. DB/model failures are distinct from reducer refusals.

- [ ] Write SQL integration tests first. The initial failing test imports the missing owner and demonstrates intended persistence:

```rust
let owner = SessionOwner::new(pool.clone(), FixtureVerifier);
let first = owner.prepare(plan.clone(), clock(1000)).await.unwrap();
assert_eq!(first.run.phase(), Phase::Prepared);
let started = owner.start(&plan.binding().unwrap(), launch(&plan), 120_000, clock(1000)).await.unwrap();
assert_eq!(started.snapshot.run.permit().unwrap().sequence, 1);
let reopened = SessionOwner::new(pool, FixtureVerifier);
assert_eq!(reopened.inspect(plan.subject.actor_id()).await.unwrap().unwrap(), started.snapshot);
```

- [ ] Run `F1A_CHECK=owner aq test docs/implementation/F1a-evidence/test_cargo.py -q --timeout=1800`; confirm missing owner API fails compilation.
- [ ] Implement SQL row locking, validated bounded JSON storage, actor/conversation/authority identity checks, and per-actor generation cap. Every successor increments once, preserves prior rows, and uses an internally derived `MailHistoryFence`.
- [ ] Implement start/renew/report transaction semantics using the existing reducer:

```rust
let transition = snapshot.run.transition(&event, clock)?;
// Persist transition.state even if transition.decision refuses a late renewal:
// expiry is durable and cannot be rolled back by returning the refusal.
save_locked(&mut transaction, &transition.state).await?;
transaction.commit().await?;
```

- [ ] Test concurrent initial preparation and successors, wrong binding/boot/regressed clock, permit limit and original deadline, late Running and renewal after cancellation/expiry, uncertain stop, verifier refusal, retained generation history, all-generation mail/cursor conflicts and cross-role operation reuse.
- [ ] Run owner integration target and complete central-authority reducer tests; fix implementation defects without weakening expectations.
- [ ] Commit the independent implementation.

## Task 2: Evidence and blocked submission

**Files:** `docs/implementation/F1a.md`, test adapter logs, and this plan.

- [ ] Run formatting and targeted clippy for changed crates and test targets through the same resource gate. Full host acceptance is unavailable; do not name a persistence-only fixture as either required host test.
- [ ] Record environment, exact commands, results, source base, implementation commit and remaining dependency in `F1a.md`.
- [ ] Check `origin/dev`; if absent, record that the instructed final merge cannot run rather than silently substituting another branch.
- [ ] Commit and publish through `aq git push`.
- [ ] Close `--outcome fail --work-outcome blocked`, explicitly naming `crisp-nexus-12`, both unmet host tests and outstanding cross-family review. This follows the operator's direction and is not a passing submission.

## Deferred integration after crisp-nexus-12

The published subject-neutral host contract must replace the incompatible v1 task-bound launch input. Only then can F1a vendor the exact host/mapper packages, implement the trusted physical-evidence verifier, create the binary/image and protected credential adapter, qualify live adoption and stop-and-preserve, and run `supervisor_session_starts_under_central_permit` and `supervisor_restart_resumes_without_duplicate_session`. No substitute remote assignment or synthetic named-test pass is acceptable.
