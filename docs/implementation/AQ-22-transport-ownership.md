# Observation transport ownership

This follow-up adds a small portable contract for central admission of a worker's
observation store. CON03 batch and receipt bytes stay unchanged. It is separate
from execution grants: neither a transport owner nor an accepted usage report
permits an agent to run, mutate the graph, or release a workspace.

## Creation and activation

One node has one durable observation transport owner. Sequence numbers span
boots. A missing store is never interpreted as an empty store.

1. Before contacting AQ, the worker durably saves a create intent with a unique
   operation ID, store ID, node and boot. The central prepare transaction checks
   current node identity, absence of any previous transport owner, and absence
   of earlier observation history. It allocates the owner ID and immutable
   allocation receipt. Repeated identical preparation returns that receipt
   alongside the current owner snapshot; changed payloads conflict.
2. The worker creates and fsyncs a private store containing the exact allocation
   marker. Prepared stores cannot admit sources, collect records, or emit facts.
   Before attempting creation it records that creation has started. An uncertain
   create with a missing file holds for recovery; an old receipt is not a local
   reseeding instruction.
3. The worker durably records activation intent and sends its marker digest.
   The central owner verifies the exact allocation and marker, then permanently
   records Active, even if its accepted sequence is still zero. After that,
   preparation replay can only describe an existing active owner. It cannot
   authorize creation.
4. If activation's response is lost, an existing exact store may query status
   and resume activation. If that store is missing, it holds for explicit owner
   recovery. Neither a status receipt nor a serialized verified-proof type is a
   universal create permission. Loss of all local files still leaves central
   ownership, so a fresh prepare is refused.

The local adapter must distinguish creation, activation and existing-store
resume proofs. It verifies replies through authenticated node HTTP and checks
operation identity, canonical request digest, exact store marker and current
snapshot. Creating from a cached historical receipt is prohibited. Filesystem
identity checks and the durable local intent remain worker responsibilities.

## Boot transitions and old observations

An active owner may advance from its exact current boot and owner revision to
the node's newly admitted boot. Permanent indexed receipt history forbids boot UUID
reuse, including the initial boot. The request binds the original store marker and the durable
worker next sequence. That sequence may exceed central accepted-through plus
one because old packets are still pending. It may not be below it, and the
central service bounds pending distance by the declared spool capacity. A boot
transition does not invent missing facts or acknowledge undelivered packets.
The worker atomically fences old live sources, preserves their exact bindings,
records an explicit source gap, and drains frozen old-boot packets in order.

Exact operation receipts are immutable and quota bounded. Replaying an older
boot receipt returns the current snapshot and cannot restore old ownership.
Revocation is checked before even an exact replay. Legacy batch ingestion is
retained only for explicitly pre-owner test fixtures; runtime source admission
requires an activated owner and a separately admitted source.

## Source policy and retained replay

Source admission is a later additive contract. It binds exact session/attempt,
source ID/generation/kind, native key, parser and actual qualified host-start
provenance. The daemon chooses account scope, native-call namespace, category
layout, precedence and completeness qualification from retained launch and
continuation records plus its catalog. Worker strings do not define policy.
Successful execution launch does not qualify a transcript parser. Initial
cumulative counters stay unknown unless their owner proves a fresh-zero or
retained-history baseline.

Reparse permission requires an owned retained artifact, exact old/new registered
parser, bounded known prior fact references and actual verified bytes. It does
not infer retention from a live filesystem path. Recovery of a missing active
store needs explicit gaps, retirement of old ownership and replay fences; that
operation is intentionally absent from this first contract.

Qualification must cover lost prepare/activation replies, lost/renamed stores,
exact and changed operation retries, activation at sequence zero, concurrent
prepares, old-boot packets after transition, revocation, receipt quotas and
refusal to create from old activation/status history. No native harness or
production observation runtime is qualified by the contract alone.

## First implementation checkpoint

The portable `aq-observation-admission0.1.0` contract implements three mutations,
a separate read-only status and a pure reducer. An immutable operation receipt
travels with the current owner snapshot; equal revisions require identical owner
bodies. Canonical digest vectors are independently reproduced with Python.

The server adapter uses the common node lock, four existing observation SQL
connections, immutable receipt rows and a monotonic owner row. Its finite bound
is one owner plus66 receipts per node (prepare, activate,64 boot transitions),
with each request/receipt SQL body bounded to4/8 KiB. That fixed metadata ceiling
is separate from the128 MiB normalized-event quota and is not a physical-disk
claim. Receipt insertion failure rolls back the entire state transition.

Migration016 and the bootstrap/status router are staged in this checkpoint.
Tests explicitly install the migration and construct its router. Source registration and batch ingestion now require an activated owner;
new sources additionally match its current registered boot. Ordered registration,
production listener mounting and the worker's durable filesystem handshake remain
required before runtime use. This checkpoint does not silently migrate old unowned sources.
