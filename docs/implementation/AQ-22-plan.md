# AQ-22 session observations and accounting

Status: implementation starting. This plan implements the accepted session
operations design, with CON03's published lifecycle protocol unchanged. It is
not proof of native harness or hard spending-limit qualification.

## Owners and boundaries

Worker readers own native format interpretation and durable source cursors.
The central service owns accepted event identities, source registration,
append-only accounting/correction facts, human inspection, retention and
pricing. Graph task completion, process stop, terminal control and wait outcome
remain with their existing owners. Ingestion never writes the task graph per
token, transcript record or heartbeat.

A small pure Rust `aq-accounting` crate handles bounded accounting decisions and
rate calculations. It receives verified policy plus exact prior facts, and
returns a proposed contribution/replacement/ambiguity result. It owns no SQL,
network, filesystem or provider credentials. The server applies those decisions
and event cursors atomically. The implementation must not load an entire
lineage history to ingest one event.

## Registration and ingestion

Accept the existing bounded lifecycle Packet/ObservationBatch contract (32
events, 256 KiB packet, 64 KiB event). Authenticate the assigned node and match
node, boot, exact immutable session, attempt generation, launch selection,
project and task lineage against the owning execution records. A worker cannot
choose a new lineage or account by writing it into a transcript. A late fact
from an expired attempt may still be admitted under retained observation
ownership; it grants no execution, graph, terminal or credential authority.
Revoked nodes cannot create new facts. A local central-host adapter uses the
same service, with an independently verified central host principal; until that
owner exists the network path refuses invented central sessions.

Source registration freezes native session key, source UUID/generation, parser
artifact, supported source/counter semantics, and an authenticated host binding.
Discovery remains worker-local and scoped. Server endpoints never fetch an
arbitrary filesystem path supplied in a record. Parser policy and category
semantics are verified artifacts; a wire reference or self-advertised capability
alone is not proof of complete accounting.

One durable node-wide transport cursor and exact event identities distinguish
packet replay from conflicting sequence reuse. An admitted boot transition
with a retained spool preserves old fact bindings and the sequence. Missing
spools require explicit source-gap recovery and retirement of old transport
ownership; never auto-reseed or relabel historical facts. Preserve the frozen batch and
its acknowledgment; no partial success acknowledgment can silently discard
accounting. Known history or corrected facts may advance transport delivery
without adding another charge. Database quota exhaustion refuses atomically;
explicit upstream gaps remain visible. Publish the exact ACK/registration seam
with WRK15 before implementing a second competing wire format.

## Call identity and corrections

A transport event ID is not a provider call ID. The accounting key uses a
verified provider/account identity and a declared native-call namespace that
survives approved history import/resume. Model names are attribution, not an
excuse to charge one native call again. Where native IDs are only session-local,
the policy includes a verified conversation identity; unrelated sessions must
not accidentally collapse coincident IDs. If an owner cannot establish identity
across sources, retain an ambiguous/unattributed observation and incomplete
accounting instead of adding two sources together or silently dropping one.

For one known call, a declared source-precedence rule selects the canonical
contribution. Identical replay adds no tokens. A higher-precedence source or
explicit correction appends an audit fact referencing the exact current prior
event/digest; it replaces the contribution atomically. Lower-precedence evidence
is retained without another charge. Conflicting same-priority reports stay
visible and require correction. Imported history in a successor session cannot
reset the original task-lineage budget. Transcript rewind or deletion does not
refund already incurred usage.

Cumulative baselines are scoped to source and counter generation. The first
counter requires declared fresh-zero or retained-history baseline semantics;
absence of a previous row is not proof that a cumulative total is new spending.
Decreasing counters without a new verified generation create a gap. Known
historical prefixes are never recounted after source rotation or parser upgrade.
Parser corrections may lower an estimate only through an explicit audited
replacement of known facts, never by deleting source history.

## Counter and cost semantics

Keep five categories independently known/unknown: input, output, cache read,
cache write and reasoning. Zero is a reported value. Category relationships are
owned by a pinned policy (for example cache read included in input, reasoning
included in output). Neither token totals nor prices blindly sum overlapping
fields. Checked arithmetic refuses overflow and inconsistent subset counts.
Each projection exposes completeness and provenance; partial evidence does not
satisfy a complete-accounting policy.

Rate cards are immutable, versioned owner configuration with currency, explicit
category mapping, integer microunits and checked deterministic rounding. No
live provider prices are hard-coded. Token estimates, subscription allocation,
local-model allocation, provider-reported charge and unknown cost remain
separate bases. Unverified worker-reported costs cannot become an invoice or a
trusted budget fact. Mixed currencies are not silently converted. Late accepted
corrections retain old rate/cost provenance and update projections with an audit
trail.

The AQ20 BudgetProvider reads this ledger and the actual admission attempt/time
owner. Retries and hibernation successors preserve lineage identity. Incomplete
or unknown charges remain explicit; no hard guarantee is advertised without a
qualified enforcement boundary and treatment of in-flight usage.

## Content, inspection and capacity

Transcript text/tool content is bounded, hash-verified artifact data with a
separate retention policy. Normalized metadata is not permission to retain
arbitrary text forever. Expiry/erasure removes content access and preserves only
policy-permitted minimal accounting identities/corrections and exact anti-replay
fences. If policy requires deleting identifying accounting evidence too, retire
that source namespace so replay cannot create a new charge. Do not retain
secret-bearing raw text under the label of an accounting tombstone.

Use AQ21's shared `session_inspection_acl` and current user authorization for
session show/logs/usage/peek; the key is the exact immutable SessionIdentity,
with expiry and explicit Viewer grants. Owners can inspect. Inspection never
grants terminal control. Lineage-wide aggregate views are initially Owner-only.
Reuse the authenticated `aq-human-http` helper and coordinate `aq-session` CLI
subcommands with AQ21. Worker and central host inspection cannot borrow broader
graph profile permissions.

Views distinguish live authority, last reported process state, provider
activity, agent progress claims, gaps and observation freshness. A bounded
terminal capture comes from AQ21's published capture path; transcript silence
or a token increment does not prove a process is dead, productive or complete.
Use separate bounded SQL/request pools and CPU admission from heartbeat,
cancellation and grant renewal. Runtime migration is coordinated after the
shared inspection ACL; pure accounting can proceed independently now.

## V1 evidence to preserve and improve

Read-only reference: `agent-queue2` commit
`9a9b8e8ef992d1ff699755a9383ed3c91e2844ea`.

- `docs/reference/usage-accounting.md`: separates observed spending from provider
  quota snapshots; preserve that distinction and explicit unpriced/unknown
  categories. Replace the unbounded token ledger and incomplete cache pricing
  with bounded admission, pinned semantics and explicit retention policy.
- `src/database/queries/transcript_queries.py`: documents repeated history being
  recharged after supervisor/session restart. Preserve cross-incarnation
  deduplication, but use scoped source identity/generation rather than a global
  filesystem-path identity or an unconditional zero-offset reset.
- `src/database/queries/token_queries.py` and
  `migrations/versions/a00000000122_transcript_usage_fences.py`: preserve exact
  replay fences when detailed calls are archived. Keep retention and replay
  admission in one transaction so deletion cannot open a second-charge window.
- `src/sessions/transcripts/watcher.py`: useful native usage-call correlation and
  provenance evidence; worker parsing belongs in WRK15 rather than a central
  scan of every provider home.

## Qualification stages

1. Pure accounting decisions: identity collisions/history replay, precedence,
   exact correction references, cumulative reset/baselines, overlapping fields,
   reported-zero versus unknown, integer pricing and overflow.
2. PostgreSQL ingress and retention: actual authenticated scoped registration,
   atomic batch/cursor/ledger writes, replay/conflict/quota races, restart and
   source namespace retirement. Keep clear synthetic versus actual harness
   provenance.
3. Authenticated CLI/API and budget adapter: independent capacity, exact ACL,
   pagination/freshness/gaps, bounded retained artifacts and deterministic rate
   revision. Verify against WRK15's real outbound journal and fake central peers.
4. Clean component/full-area evidence and implementation-plan update. Actual
   selected four-harness qualification remains INT03; no model calls or messages
   to humans are necessary for this task's deterministic tests.
