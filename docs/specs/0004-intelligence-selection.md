# Central intelligence selection (AQ-20)

The independently testable `aq-selection` crate resolves explicit AQ-18 policies
against trusted project, installed harness/credential, availability and budget
facts. The server module `intelligence` owns persistent facts and metadata
publication. Scheduling does not probe a model provider.

## Catalog policy and compatibility

`Intelligence.selection_policy` is optional only for artifact compatibility.
Absence gives `selection_policy_missing`, not guessed defaults. Adding/changing a
policy changes its definition and requires a new intelligence identity/version
under the existing immutable catalog rules. The retained pre-policy golden
catalog roundtrips byte for byte with its original digest. Model reasoning is
mapped by exact provider/model/reasoning-reference through a qualified native
adapter; no cross-provider equivalence is assumed.

Explicit fallback reason allowlists control advancement after missing credentials,
unavailability, unsupported settings, stale/unknown facts or policy denial.
Policy bounds retry count, elapsed lineage time, observation age, output tokens,
capabilities, currency/cost and unknown behavior. Project and native limits only
narrow effective resources. Unknown availability never authorizes missing keys.

## Startup configuration

Optional `[intelligence]` in the vault has a positive monotonic `revision`, at
most 32 `credentials` and 16 `qualified_harnesses`. Each credential entry
(`CredentialConfig`) contains an `AuthorizedCredential`, optional `kind`, optional
`allowed_backends`, `allowed_node_ids` and optional absolute `secret_file`.
Its profile is an exact artifact reference; provider/account/endpoint are logical
references. `credential.provisionable` must be false in configuration: startup
loads derive it from enabled state and protected key presence.

`kind` is the closed `api-key` (default when omitted) / `subscription-login`
enum. It is operator metadata only; neither kind changes runtime, key-presence or
delivery behavior. `allowed_backends` lists distinct values from the closed
`docker-local`, `boat` and `cloudflare-containers` set. Omitted, it defaults to
all three: an unrestricted credential policy, with placement still deciding which
backends are enabled. Operators can narrow it; an explicit empty list denies every
backend. Unknown values, duplicates and more than three entries refuse config.
`allowed_node_ids` remains an independent placement restriction: a credential must
pass both. The nested `credential.allowed_backends` is not a second operator knob.
It must keep its unrestricted default in configuration, and `load` copies the
entry-level list into the loaded `AuthorizedCredential`.

The registry configuration digest preserves the legacy digest for unchanged
default configurations (`api-key` with all backends allowed), so upgrading needs
no config `revision` bump. Changing `kind` or the effective `allowed_backends` set
needs a newer config `revision`; registration refuses a same-revision conflict.
Backend sets are order-insensitive: reordering a list does not change the digest,
and explicitly listing all backends is equivalent to omitting `allowed_backends`.

`CapabilitySnapshot.backend_kind` is an optional trusted node fact, never a
caller-supplied placement hint. A credential with all three backends allowed
remains eligible when it is absent. For a narrowed credential, an absent backend
gives `credential_backend_unknown`, and a known backend outside the list gives
`credential_backend_not_allowed`. Both are candidate state `PolicyDenied`; only the
policy's explicit fallback allowlist can advance to the next candidate. Default
snapshots and credentials omit these values when serialized, preserving the
historical selection vectors. Central selection has no backend identity, so
narrowed credentials always fail closed there. Material provisioning and issuance
independently apply the same check before reading private material. A selector
result alone cannot bypass issuance policy.

Existing server inventory and issuance adapters intentionally supply an unknown
backend. Consuming the authenticated B1/C4 backend advertisement is not
implemented here. Until C4 supplies trusted backend facts at each boundary,
unrestricted credentials keep working and narrowed credentials fail closed.

Key-presence facts are captured once during config deserialization, with no-follow,
nonblocking opens, regular-file/owner/mode checks and a 64 KiB cap+1 read. Missing
keys stay missing; unsafe, empty or oversized keys refuse config. No key bytes
enter selector inputs, inspection output, decisions, prompts or manifests. Private
rotation fingerprints remain server-side. Secret bytes are discarded after this
check; a provider credential delivery adapter is not implemented here. AQ-08
must require its actual credential owner to revalidate exact revision/material
against the captured fingerprint (or supply a sealed secret lease) before
provisioning; a startup presence fact is never sufficient launch authority. Restart with increasing config and
credential revisions after rotation. Removing/readding a profile cannot reset
its historical version or credential revision. No secret management provider or
provider credential registry service is introduced.

Qualified harness definitions contain native mappings, exact CLI/adapter and
environment artifacts, tested capabilities/limits and an optional error
classifier artifact. These are operator-managed qualification references;
synthetic tests are not evidence that a real external adapter works. Hard cost
remains unavailable until a real native enforcer **and** ledger adapter qualify.

## Actual HTTP adapters

All owner mutations use normal owner authentication and CSRF enforcement. These
routes exist on the admin listener:

- `POST /api/intelligence/register` with `{}` installs the immutable startup
  snapshot, refusing stale/conflicting config revisions. It performs no key or
  provider I/O. GET does not mutate configuration.
- `GET /api/intelligence/registry` returns current nonsecret credential facts,
  bounded observations and explicit accounting/hard-cost unavailability.
- `POST /api/intelligence/observations` accepts operation ID, expected registry
  revision, increasing source sequence, exact known credential scope, finite
  state, TTL and cooldown. Principal and receipt time come from authentication and
  the server clock. Model/node omission permits deliberate owner account scope;
  a supplied node must be in current operator credential placement.
- `POST /api/intelligence/explain` accepts `catalog_artifact_json` and explicit
  selector `inputs`. It returns `authority_admission:false` and a deterministic
  snapshot explanation. These caller snapshots never become durable launch
  authority or replace current project/credential/accounting facts.

Machine routes on the worker listener use existing node credentials:

- `POST /v1/worker/intelligence/inventory` accepts operation/boot IDs, increasing
  sequence, bounded TTL, known qualified harness artifact references and exact
  installed credential profile/revision/presence. Current registered boot and
  owner placement are rechecked under the node lock. A presence claim cannot
  authorize provisioning or create a provider availability observation.
- `POST /v1/worker/intelligence/failure` accepts AQ-owned `StructuredFailure`
  schema version 1: operation/attempt/selection/node/boot/classifier, sequence,
  finite failure classification and optional lifecycle event provenance. The
  daemon derives account/endpoint/model/credential scope from the actual retained
  sealed attempt selection. Classification must arrive while that exact attempt
  is still granted and unexpired, before its final result report; AQ-08/WRK08
  must preserve this ordering. Generic lifecycle errors are insufficient. Legacy
  fixture attempts have no such binding and are refused. There is no route to
  manufacture a sealed selection binding.

JSON mutation records deny unknown and duplicate known fields. Scope/sequence
receipts survive TTL expiry; expiry does not permit replay or erase cooldown
history. Owner operation receipts permit exact replay after configuration
changes, under current owner authorization. Worker receipts additionally require
current node authorization/boot; they do not resuscitate stale incarnations.

Observation scopes are exact provider/account/credential revision/endpoint, plus
optional model and node. A fresh failure applies only to matching scopes. Old
worker boots are ignored during selection. Increasing source sequence (before wall-clock tie breaks) supersedes the same
source/scope, while independent restrictive evidence remains conservative. TTL
and cooldown are at most one hour; worker classifier feedback uses server-owned
30-second TTL/cooldown.

Retention refuses further mutation at 1,024 observation scopes, 10,000 owner
receipts or 16 MiB receipt JSONB; machine receipts share 4,096 entries/16 MiB per
node and 128 KiB per snapshot. Credential definition history is bounded at 1,024.
Decision metadata is bounded at 1,024 artifacts, 8 MiB per artifact and 64 MiB
retained JSONB. Checks include the incoming uncompressed JSONB size; exact replay
does not consume capacity. No silent eviction removes a replay fence.

## Trusted resolution and actual admission boundary

`intelligence::resolver::resolve_trusted` accepts an internal authenticated task
aggregate, real AQ-19 frozen project binding and AQ-18 compiled compositions. It
loads the active retained catalog, server credential config, current installed
snapshot, scoped observation registry and actual attempt count. It writes an
immutable decision with stable operation replay. The startup-only credential
loader means resolution performs no filesystem secret reads or provider calls.

The bootstrap accounting adapter explicitly reports unavailable, with no hard
budget provider or estimated provider rate card. First-attempt retry/elapsed
facts can be known; existing legacy attempts have no invented latest-authority,
stop/retention/reconciliation or ledger proof, so replacement is conservatively
refused. A policy demanding complete accounting or hard cumulative cost refuses.
The pure provider interfaces permit later authoritative AQ-22 adapters; estimates
are never actual charges and unknown usage is never converted to zero.

A retained decision is **not runnable authority**, including on exact replay.
AQ-08 owns authenticated current task/context/outcome/access-profile inputs,
actual assignment/session/source artifacts and final selection-to-offer sealing.
It must use `fence_for_offer` inside its SERIALIZABLE offer transaction, retain
all locks, atomically persist the exact selected tuple/composition/artifact
bindings and attempt budget transition, check final time and retry serialization
failures. The fence refuses altered/unretained decisions, missing eligible
selection, stale boot/capabilities/credentials/relevant observations/project/
catalog/retry count or expiry after lock waits. It uses node -> project -> catalog
-> registry ordering and has no external I/O. Unrelated account observations do
not invalidate unrelated credential scopes. Global registry revision is
provenance, not a universal placement invalidator.

Lifecycle lineage UUID/checkpoint references are provenance; every continuation
maps to original `(project, workspace, task)` accounting authority. The current
project admission adapter is worker-only. Central supervisors need a distinct
central authority adapter; they must never fabricate a worker node ID. Current
human graph-access API01 profiles/digests remain separate from catalog,
composition and credential references.

## Qualification boundary

Pure selector tests and synthetic PostgreSQL/authenticated HTTP tests cover order,
reasoning mappings, fallback/unknown behavior, scoped cooldown, key presence,
credential backend policy, replay, budget refusal and final admission fences.
Synthetic credential bytes, native qualification artifacts and classified failures
are test data. No paid model calls, actual provider quota/authentication behavior,
real native reasoning mapping, cost-enforcement adapter, container backend
execution or actual profiled launch is qualified here.
