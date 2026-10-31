# AQ admission contracts 0.1.0

Pure versioned preparation contracts and bounded placement proposals. This crate
contains no daemon, database, network client, provider call or secret delivery.
Types and structural validation confer no authorization, reservation, retention
or stop proof. Runtime owners authenticate facts and enforce the final transaction.

`MaterialLease` binds the existing session `LaunchBinding` and exact credential
profile/revision/provider/account/endpoint/purpose, bytes/digest and bounded time.
It is private authenticated delivery metadata, never a prompt/public profile.
Only the real owner may issue/rotate/revoke it. Byte verification is mandatory;
worker presence alone does not establish account identity. `MaterialReceipt`
identifies an attempt-private binding artifact, not a filesystem path or launch
grant. Provider material and delivery routes are not installed by this package.

`SourceAdmission` requires explicit immutable object or fully qualified safe ref,
exact public repository generation, source-policy artifact and bounded original
observation time. `RetainedArtifact` verifies exact bytes/digest/size and retention
metadata; its receipt reference still needs authenticated owner verification.
No placeholder source/outcome/context artifact is synthesized. The initial helper
caps the total retained descriptors at 128, including definition references; it
can refuse a public-envelope-valid manifest with 128 distinct context artifacts
plus definitions. This is an explicit smaller admission profile, not full maximum
public-envelope support. Callers must refuse the unsupported larger profile.

Placement proposes bounded per-project round-robin assignments, preserving
priority and stable ties. Resource fit precedes truncation, and earlier oversized
work cannot hide a later fit. Warm source state is only a preference among
eligible nodes. Cold eligible nodes may prepare. Expired/foreign-boot capacity
is not used. Proposals explicitly carry `authority_admission:false`; the daemon
must independently claim and fence actual resources and worker admission.

`artifacts/contract.schema.json` and `fixtures.json` are independently packaged
publications with drift tests. The pinned shared launch fixture is copied from
session protocol 0.1.0 and is not a new version of that protocol. Schemas include
field/array/UTF-8 bounds; Rust validation additionally checks relational identity,
time/freshness, role, object-format, capacity and scope invariants. No schema can
establish live authorization or retention. Receipts and leases need real owner
records, not caller assertions.

Run `cargo test -p aq-admission --locked --offline`. For standalone package tests,
provide the separately pinned public `aq-session-protocol =0.1.0` artifact through
Cargo's normal registry/patch mechanism; no sibling worker source checkout is
needed. In this development workspace, packaging uses the retained public vendor:

```sh
cargo package -p aq-admission --allow-dirty --offline \
  --config 'patch.crates-io.aq-session-protocol.path="/absolute/public/vendor/aq-session-protocol-0.1.0"'
```

Publication generation: `cargo run -p aq-admission --example export -- schema`
and `... -- fixtures`. New incompatible fields/semantics require a new immutable
contract version and coordinated consumers; do not edit pinned session bytes.
