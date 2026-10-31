# M4 graph and source foundations

This increment implements AQ-06, WRK-06 and AQ-09. It does not close GATE-04:
the outbound transfer client, production agent launcher, durable graph mutations
and final task-result policy are subsequent tasks.

## Component boundary

The daemon keeps agent authority, graph access and central source receipts. The
worker owns compute admission, guarded processes, local source workspaces and
protected pending artifacts. Workers have neither a graph database nor graph
service credentials. No broker or additional database is introduced.

The product consumes an immutable worker source archive and the published
`aq-worker-protocol 0.2.0` package. Foundation wire protocol 1 and execution wire
protocol 2 remain unchanged. Optional frozen source/graph bindings and explicit
capabilities extend the execution specification; old unbound fixtures keep
their existing canonical digests. The protocol package was verified using
`cargo package` before vendoring. Ordinary builds need no sibling repository.

| Component | Selected identity |
| --- | --- |
| Worker implementation | `e7c99b7ce1c31711f23ccb9d4410c406e062108f`, version 0.3.0 |
| Shared protocol package | `c3af96037083508f5cb586861ca0bbb615569f16`, version 0.2.0 |
| aq-git | `f3660a89cbeac0e068105af2926dfdcb06a1daad`, version 0.1.0 |
| aq-graph | `225329cd6acf5a775fd2a0b016dd1c45fb16c79b` |
| Qualified graph image | `sha256:807beba98dcd20b0ae335f09cae8c06ecb182a352ee78de278cb47fccb2ee3be` |

The daemon migrates state format 2 to 4 through two additive migrations. The
worker migrates journals 1/2 to format 3, preserving existing process evidence
and reports. Old workers must refuse the new format without modifying it.

## Agent authority and graph reads (AQ-06)

Trusted provisioning seals a profile and context against the attempt's frozen
digests. The assigned node obtains a short-lived session credential using an
explicit credential-version compare-and-swap. Only its verifier is persisted;
lost issuance requires explicit rotation. Authority checks bind node, boot,
session, generation, cancellation, revocation and grant expiry. Reads recheck
authority after upstream I/O and before returning data.

The gateway implements all six API-01 read shapes. It filters nodes,
relationships, search and history before returning authorized projections;
cursors bind the exact query, attempt and credential version. Response size,
scan count and execution duration are bounded, and partial coverage is explicit.
Full context and issuance envelopes also obey the response budget. Upstream
API-version headers are mandatory; private error bodies are not forwarded.

See [operator interface](../operations/agent-access.md) and the
[AQ-06 qualification](../qualification/aq06-agent-access.md).

## Local workspaces and storage (WRK-06)

The worker invokes aq-git's public local socket interface. Stable operation IDs
and payloads are journaled before dispatch. Exact-base workspaces are prepared
before guardian launch; checkpointing waits for stopped-writer proof. Ambiguous
component outcomes preserve reservations and source data. Retiring source
requires both central retention and explicit result retention release.

The initial storage profile supports 64 MiB bundles and 512 MiB expanded source.
It charges repository residency once per repository, reserves per-attempt
workspace/artifact headroom, accounts for actual pending bytes, and measures
free space on the aq-git volume. Retired cache remains charged until eviction.
Unexpected growth stays protected and stops admission; these bounds are not OS
filesystem quotas. Rehydration after eviction requires a fresh exported transfer,
because an old import receipt is historical evidence rather than a cache refill.

The configured CLI runs the synthetic guardian inside the prepared worktree and
collects a stopped checkpoint/export. It emits `source_pending` until retention
is established. Only configured, verified nodes advertise source workspace
support; no node advertises automatic HTTP source transfer yet. Detailed limits,
CLI options and standalone tests are in
[aq-worker's source adapter document](../../../aq-worker/docs/source-workspaces.md).

## Central source retention (AQ-09)

Authenticated bounded endpoints reserve, upload, inspect and finalize source
transfers; the owner can also provision a frozen base for bounded download.
The daemon derives repository and attempt authority from immutable assignments.
Workers cannot supply arbitrary Git commands, filesystem paths or remote URLs.

SQL reservations precede filesystem effects; chunks are fsynced before offsets
are acknowledged. A durable import operation ID survives uncertain responses.
Hashing and component I/O run outside execution authority locks. Pending imports
are protected from expiry cleanup. Only a verified aq-git import receipt permits
central retention acknowledgement and staging release. A late attempt can retain
its own source without restoring execution authority. After final retention
release, new transfer intents are refused while existing retries remain usable.

The transport transfer UUID and aq-git artifact/retention UUID are distinct.
The transport manifest digest hashes exact file bytes; the aq-git receipt digest
hashes its canonical typed manifest. Artifact result references use exact-byte
digest, total manifest/bundle size and the durable central receipt ID. Result
admission checks all frozen bindings and outstanding transfers before releasing
the task claim. Execution evidence is monotonic: a later `not_started` report
cannot erase running, launch-uncertain or earlier stopped evidence.

See [source retention and startup instructions](../source-retention.md).
The server currently connects to an explicitly managed private aq-git process;
automatic central process composition is REL-01 work. The worker already owns
its explicitly configured local GitService lifecycle.

## Packaging and qualification boundary

Server and worker images contain the same pinned aq-git source build and a
relocatable Git 2.56.0 runtime. `/usr/bin/git` resolves to that runtime, including
for components that use an absolute default path. The standalone build needs
at most GLIBC 2.34 and runs on Bookworm. CI and release checks reject source
checksum drift and divergent copied build stages.

Qualification combines independently tested modules, the real selected graph
and aq-git processes, actual worker CLI execution, production image assembly,
and the existing recovery and updater suites. Exact commands, counts, source
inventories and artifact identities are retained in the companion qualification
record. Historical M3 acceptance evidence remains unchanged.

No production deployment, hosted CI execution, large-fleet performance result,
hostile-process isolation, hard disk quota, live process reattachment or physical
power-loss guarantee is claimed. WRK-05/07/08 and AQ-07/08/10 still supply the
remaining agent launch, transfer, mutation, scheduling and completion path.
