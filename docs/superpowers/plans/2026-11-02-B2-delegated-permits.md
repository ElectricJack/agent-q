# B2 delegated permits implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan in this held worktree. Steps use checkbox syntax.

**Goal:** Issue durable attempt-scoped permits and enforce them across runner operations, including reboot re-adoption and server restart.

**Architecture:** PostgreSQL owns permit metadata and operation replay records. A server-private derivation key reproduces credentials for exact issuance replay; PostgreSQL stores only verifiers. Authentication must recheck current node, boot, attempt, grant and cancellation state in the transaction authorizing effects. A permit must never become an unrestricted node credential.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, pinned aq-admission-sandbox 0.3.0.

**Spec:** AQ task noble-summit-37.2 and `2026-11-02-v2-full-application-tasks.yaml`, B2. Operator message `msg-b5ea45f061ad46a5be367c80de20b495` additionally assigns registration-v2 and initial permit transport publication to B2. The aq-worker Controller/PermitProvider consumer must be coordinated in its owning repository.

## Global constraints

- Builds use the committed lockfile with `--locked`; no sibling path dependencies, submodules or floating release inputs.
- Wire changes publish schemas and fixtures from the contract owner; both endpoints consume a pinned version.
- aq-graph owns task state; AQ owns grants, attempts and dispositions.
- Runners initiate connections; node credentials never enter sandboxes.
- Stopped-writer proof precedes release or replacement. Unretained work is preserved.
- No secrets in templates, manifests, prompts, logs, assignments or task descriptions.
- Unsupported required capabilities refuse explicitly.
- No edits to architecture or specification documents in this task.
- Evidence belongs in `docs/implementation/B2.md`. Required named tests must run and pass, not match zero tests.
- Cross-family adversarial review and guarded branch publication precede passing close.

## 1. Durable permit model and credential replay

Files: create `crates/aq-server/src/delegated_permits.rs`, `crates/aq-server/src/delegated_permits/model.rs`, `crates/aq-server/src/delegated_permits/store.rs`, `crates/aq-server/tests/delegated_permits.rs`; register the module in `lib.rs`.

Interfaces: `PermitRecord` stores the owner `PermitBinding`, scope, issue/expiry/hard deadlines, verifier and revocation. `PermitRecord::authorize(&PermitContext, RunnerOperation, now_ms)` returns an explicit refusal for scope, binding or deadline failure. `CredentialKey` derives a domain-separated HMAC credential from a unique permit ID. It is private server configuration, never a database field or request value.

- [ ] Write failing tests for foreign attempt, stale boot, preparation/execution scope, expired or revoked authority, credential mismatch and hard deadline changes. The scope test must exercise stored server state, not merely the owner's fixture validator.

```rust
assert!(record.authorize(&foreign_context, RunnerOperation::Observe, now).is_err());
assert!(record.authorize(&current, RunnerOperation::Started, now).is_err());
assert!(record.authorize(&current, RunnerOperation::Observe, record.expires_at_ms).is_err());
```

- [ ] Run the focused test through an AQ pytest adapter and retain the failing log.
- [ ] Implement metadata validation using the pinned owner rules; credentials use keyed derivation and constant-time verifier comparison. Exact operation replay compares retained request digest and never extends deadlines.
- [ ] Run the focused tests, including credential redaction and replay under a different derivation key.

## 2. PostgreSQL integration and revocation

Files: create `crates/aq-server/migrations/0048_delegated_permits.sql`; modify `store.rs` migration registration and `lib.rs` state format. Add durable-store tests to `tests/delegated_permits.rs`.

Interfaces: transaction-owned issue/load/revoke/update functions operate on `sqlx::Transaction<Postgres>`. Uniqueness is `(node_id, operation_id)`, one live identity per attempt/generation, and unique channel. Request digest, original metadata and verifier are immutable issuance evidence; boot/scope/current expiry change through guarded updates. Rows carry no raw credential.

- [ ] Test exact issuance replay, changed-request conflict, concurrent operation serialization, rollback, process/store reconstruction and revocation.
- [ ] Add the migration with foreign keys to the node and attempt, positive bounded timestamps and scope checks.
- [ ] Recheck node and hot authority under locks before issuing or authorizing; refuse cancelled/revoked attempts and expired grants.
- [ ] Integrate node revocation and B3 cancellation without modifying lifecycle ownership or material-lease records.
- [ ] Validate reconstructed rows on load and test corrupted identity/deadline refusal.

## 3. Published transport, issuance and upgrade

Files: `profiled_execution/registration.rs`, `accepted_preparation/accept.rs`, shared `phase_execution/owner_core.rs`, new `delegated_permits/api.rs`, versioned transport artifacts under `protocol/` only after ownership coordination.

- [ ] Resolve the owner-contract handoff gap recorded on the task: phase acceptance/grant replies currently deny unknown fields and have no permit field; Prepared/mail have no operation variants. Publish/re-pin the agreed contract and fixtures in coordination with aq-worker. Do not silently map Prepared onto material fetch.
- [ ] Authenticate registration-v2 with a node credential; validate the pinned `Registration` envelope and refuse unsupported required capabilities. Persist exact advertisement/replay alongside current native registration.
- [ ] Issue preparation permits in the same acceptance transaction, binding the actual sandbox and the stable assignment generation. Return the agreed published handoff.
- [ ] Upgrade scope and expiry only inside the transaction committing the phase grant; preserve the hard deadline and operation replay guarantees.
- [ ] Test acceptance/grant replay, changed bytes, wrong node/boot, unsupported capability and failure rollback. Notify the controller owner of exact published routes and fixture revision for its PermitProvider adapter.

## 4. Runner route authorization

Files: `worker_api.rs`; material, phase preparation/Started/Stopped, renewal, artifact/source and results transports; observation batch, terminal relay and message transports.

Interfaces: an opaque `AuthenticatedRunner` contains a permit identity and bound attempt, never node credential bytes. Each typed route supplies its operation and attempt/channel binding. The effect-owning transaction rechecks this evidence after waits. Node-only registration/poll/accept continue to reject permit credentials.

- [ ] Add failing route tests for foreign attempts and every operation/scope mismatch before mounting permit authentication.
- [ ] Connect each route listed in B2 explicitly. Validate every member in batched observation/terminal traffic; empty batches still authenticate.
- [ ] Restrict terminal polls to the permit channel and keep aggregate accounting keyed by node across its sandboxes.
- [ ] Keep material lease ownership changes outside this task; do not weaken existing lease verification.
- [ ] Verify legacy node routes and the explicit refusal of permit credentials on registration, poll and accept.

## 5. Renewal, re-adoption and restart

Files: `delegated_permits/readoption.rs`, `delegated_permits/renewal.rs`, `profiled_execution/renewal/api.rs` and focused PostgreSQL/HTTP tests.

- [ ] Implement `node_reboot_requires_readoption_before_renewal_extends`: change the node boot, verify renewal returns awaiting re-adoption without modifying expiry, re-adopt the exact attempt/generation/sandbox, then renew.
- [ ] Use the pinned request/response validators; node-authenticated re-adoption updates boot only, preserves deadlines and checks operation replay before effects.
- [ ] Renewal compares immutable grant origin and revision with storage; default extension is 120 seconds, bounded by the hard deadline and current authority. Expired/revoked authority tells the runner to stop and preserve.
- [ ] Implement `server_restart_mid_attempt_resumes_or_preserves` using reconstructed server/store state: in-window renewal succeeds, out-of-window renewal refuses extension, result replay does not add a second result.
- [ ] Run the three named deliverable commands and verify actual matching test counts.

## 6. Validation, review and publication

- [ ] Run formatting, relevant Clippy and focused/area Cargo tests through AQ resource controls. Record unavailable workspace checks and existing baselines honestly.
- [ ] Write `docs/implementation/B2.md` with exact commands, results, environment, commit and remaining limits. Reconcile every route and operator-added requirement.
- [ ] Squash local leaf implementation history as instructed, publish via `aq git push`, and request the required cross-family review through the supervisor. Resolve findings against the final SHA.
- [ ] Merge the authorized integration branch, rerun affected tests, push and open the required PR. Close only with truthful deliverable evidence; incomplete work is not a passing result.

## Blocked checkpoint

The core/model and verifier-only PostgreSQL persistence are implemented and
tested, including concurrent replay, rollback/reconnect, node/B3 revocation and
storage safeguards. The storage filename is `delegated_permits/storage.rs`.
The final serial focused/area run passed 11 tests. The checklist above remains
open for full acceptance: current authority integration, owner transport
publication, every runner route, reboot/restart scenarios and review are absent.
See `docs/implementation/B2.md` for exact limits, failures and verification.

## Resumed implementation (2026-11-02)

The owner prerequisite is published at a2ff7454a7851ad4662308bc55b23254c6524ca4, admission 0.3.0. Consume its immutable archive, including Prepared/mail operations and sibling issuance/upgrade envelopes. Migration 48 preserves the backend checkpoint after main took 46 and 47 for F1a (operator message msg-a429cdbedfe2460bb1a536a2e0ae45bd). Existing immutable permit issuance records remain replay evidence; mutable authority belongs in a separate current-state record.

- [ ] Verify migrated storage and cargo check, then run the updated owner fixtures.
- [ ] Publish registration, issuance, upgrade and re-adoption transports using the owner types. Correlate issuance and upgrade with stored frozen receipts.
- [ ] Add a runner authentication context that retains attempt and operation constraints through effect transactions; node-only routes keep node-only authentication.
- [ ] Test reboot and process reconstruction with an injected clock, then targeted route scope and result replay.
- [ ] Publish final evidence and request the required cross-family review through the supervisor.
