# CON-02: profiled launch and terminal contracts

Qualified 2026-10-30/31 against worker contract commit
`7617866dd8beeb759aa6eadd7e9229d352c1c344` and human API/pinning commit
`80e66ec` in agent-q. This closes contract publication, not real harness hosting,
profile resolution, terminal transport, rendering or deployment.

## Published artifacts

- `aq-session-protocol =0.1.0`, owned by aq-worker, is an independent pure Rust
  package with bounded launch/terminal types, validation, canonical digests,
  JSON Schema, fixtures and deterministic conformance oracles.
- `aq-session-api =0.1.0`, owned by agent-q, defines future human profile and
  watch/control/message request/response shapes using the pinned shared types.
- [Package inventory](../../vendor/session-contract.json) pins the clean Cargo
  archive, source revision and every extracted file digest. Normal consumer
  builds require no sibling checkout. `scripts/check-session-contract.py`
  rejects archive, source, revision and inventory drift; CI runs it.
- [Human publication](../../protocol/session-api/v1/README.md) names future
  routes/CLI mappings and the authorization checks required of their handlers.
  The package does not install those handlers or grant authority.

The offer/accept/grant/journal binding freezes the manifest digest, execution-spec
digest, distinct graph-access-profile digest and exact remote node/boot/session/
attempt generation or independent central authority identity. Required capability
versions include profiled launch, terminal hosting and the common harness minimum.
Unknown requirements refuse. Existing execution/foundation contracts and fixture
digests are unchanged; the currently pinned synthetic worker does not advertise
profiled launches merely because the new package exists.

Terminal input has bounded identity-preserving replay, controller fencing and
expiry. Unknown injection cannot replay automatically. Resolution requires a
new capture request bound to the current controller; an arbitrary old snapshot
does not establish a fresh view. Resize keeps bounded operation history and
cannot reuse an old ID after intervening resizes. Snapshot watermark/gap and
buffer oracles establish contract semantics, not a qualified tmux renderer.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p aq-session-protocol --locked` | 19 tests pass, including independent review regressions |
| `cargo package -p aq-session-protocol --locked --offline` | Clean committed package builds independently |
| `cargo test --manifest-path target/package/aq-session-protocol-0.1.0/Cargo.toml --offline --locked` | All 19 tests pass from the extracted package |
| `cargo test -p aq-session-api --locked` | 12 tests pass against the exact pinned package, including shared fixtures and API-01 namespace separation |
| Strict Clippy for both new packages | Pass, all targets, warnings denied |
| Independent JSON Schema validation | Six worker documents and twelve human examples pass |
| Package integrity negative checks | Modified source, extra file, altered archive, changed revision and incomplete inventory all refuse |
| Existing execution protocol tests | Pass; old schema/fixture bytes unchanged |
| `python3 scripts/check-v2-plan.py` | DAG, requirements and generated Markdown consistent |

Independent review corrected missing launch incarnation binding, required baseline
capabilities, nil identities, duplicate JSON fields, resize ID reuse, controller
lease identity and stale uncertainty-resolution captures. New tests cover these
failures. Validation does not substitute for the future daemon's authenticated
current-authority checks, local permits or actual process-ownership tests.

The broader worker suite exposed a pre-existing inherited-descriptor flock race.
A deterministic held-fork-child test reproduced it before the fix. Worker commit
`d397568` explicitly releases the journal lock on orderly owner drop while
preventing a forked copy from unlocking the parent. The complete worker workspace
suite and strict worker Clippy then passed; the existing explicit aq-git binary
qualification test remains ignored without its configured binary. This separate
runtime fix is not included in the older product worker candidate yet and does
not alter the published session contract package.

## Next work

AQ-18 consumes these contracts for deterministic profile composition. WRK-10
implements the actual guarded tmux host. AQ-21/WRK-14 provide relay/bridge runtime;
INT-02/03 qualify real terminals, harnesses and operational behavior. V1 reference
selection is recorded in the [pinned implementation map](../research/2026-10-30-v1-implementation-reference-map.md);
its entries remain pending until the corresponding behavior is implemented.
