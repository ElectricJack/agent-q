# Retained inline artifacts and assigned-worker transfer

The daemon now has an actual retention owner for bounded nonsecret catalog,
context, composition, selection and outcome bytes. It stores the bytes and a
canonical receipt in one transaction. The receipt binds the actual artifact row
ID, project, owner, reference, kind, byte count and original retention interval.
Its digest forms the published `RetainedArtifact.retention_receipt` reference.
The owner neither invents Git bundle receipts nor stores provider credentials.
A retained source-observation document is Context data; it is not a retained
source bundle or permission to launch.

`artifacts::retain` takes a stable operation/artifact UUID, project, owner kind,
exact public artifact reference, bytes and absolute retention end. The caller
performs this independent transaction before acquiring node/project admission
locks. Exact retries return the original descriptor; changed reuse conflicts and
expired records cannot be refreshed implicitly. Missing or forged receipt rows
cannot commit. Reads recheck the canonical receipt, complete descriptor, body
length and digest. Composition ownership can retain both a composition and its
derived definition artifacts, including preselection policies and resolved graph
profiles. This ownership label does not itself authorize graph access.

Migration 020 makes artifact and receipt rows, and attempt links, immutable.
Inline insertion validates body length/digest and descriptor identity. Deferred
checks require the matching real receipt at commit. Quota admission serializes
independently of execution locks: at most 512 retained rows and 64 MiB of inline
body bytes per project, and 4096 rows and 256 MiB globally. Source metadata rows
also consume row capacity. Non-source rows require inline bytes. Repeatable-read
quota insertion is refused; read-committed or serializable admission is required.
No unresolved or expired history is evicted to admit more work. These are logical
bounds, not measured physical PostgreSQL or memory limits. Garbage collection,
quota configurability and sustained production retention remain separate work.
Pre-offer orphaned artifacts still count toward these limits.

`link_locked` consumes actual retained row IDs while the caller holds its
node/attempt admission locks. It verifies project identity, bytes and the required
retention horizon. SQL independently checks the actual profiled attempt's project
and held claim, and limits an attempt to 128 links. Linking acquires no global
artifact quota lock. Source-retention metadata can remain represented by the
separate source owner; this inline helper refuses Source-kind data and cannot
mint or serve its external bundle.

`POST /v1/worker/profiled/artifact` consumes the frozen `ArtifactFetch` contract.
It requires current machine credentials, the exact registered native runtime
with ArtifactTransferV1, the entire sealed LaunchBinding and a current held
assignment. Offered work uses its original offer expiry; Preparing work requires
the original accepted PreparationPlan and logical owner; Granted work uses its
current execution authority. Cancellation, expiry, missing links, ambiguous
references and foreign projects refuse. The final clock check follows all owner
locks, so a lock wait cannot extend authority. Neither a reference nor a retained
receipt by itself grants access.

Success is an octet-stream with `Cache-Control: no-store`, a decimal
`aq-artifact-offset` header, and URL-safe base64 without padding of the existing
RetainedArtifact in `aq-artifact-retention`. That encoded header is capped at
4096 bytes; requests are capped at 16 KiB. Each response returns up to the
requested 256 KiB, clipped at the original artifact end; an offset at or beyond
the end is rejected. The worker must reassemble the exact full length and verify
its digest before accepting the artifact. Partial bytes are not a verification
receipt. This initial implementation validates the complete stored blob, bounded
to 2 MiB, for each range, then returns the bounded slice. It does not claim a
large-artifact streaming performance qualification. The route shares bulk
admission and ordinary database capacity, preserving execution-control lanes.

State/container format advances to 20. Older unowned inline rows are preserved;
the new owner refuses missing receipts rather than manufacturing authority for
historical data. Tests use synthetic nonsecret data and real disposable
PostgreSQL/router transactions. They cover immutable bytes/receipts/links, exact
and conflicting replay, absent receipts rolling back insertion, Source-kind
refusal, project and retention scope, concurrent final-slot quota admission,
capacity refusal preserving exact replay, range reassembly, altered full binding,
current credential checks and an observed artifact-lock wait across offer expiry.
Native offer production and actual harness startup remain separately tracked.

## Qualification checkpoint

Clean source `ce1214f` passed 678 workspace tests with six declared optional
tests ignored, strict workspace lint and formatting. The retained
[AQ-08-inline-artifacts-qualification.json](AQ-08-inline-artifacts-qualification.json)
binds source and raw logs, including the earlier missing state-format footer
failure and successful full rerun. Independent read-only review found no blocker
within this bounded retention and range-transfer scope.

Trusted producer links define the artifact allowlist; the route does not require
every linked reference to appear in the sealed manifest. Workers cannot create
these links. Nonsecret content classification and typed catalog validation are
producer obligations. The four focused owner/HTTP tests directly exercise Offered
transfer; Preparing and Granted branches have been inspected but are not qualified
by those tests. The helper requires caller node/attempt locks as documented; its
Rust signature does not enforce that precondition.
