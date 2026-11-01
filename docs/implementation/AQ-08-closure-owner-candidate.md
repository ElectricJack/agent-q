# Granted launch closure owner candidate

This owner fences an issued native grant that has no centrally admitted Started.
It records `launch_closed_awaiting_settlement`. It does not claim that an agent
never ran, synthesize Started/Stopped/NeverGranted, clear `holds_task`, settle
source/effects, delete material/workspaces, or authorize another launch.

The default production worker router does not mount the route. A trusted
embedding must explicitly construct a bounded `HistoricalVerifierPolicy`,
install its monotonic revision/digest, then inject the returned opaque policy
through `build_worker_router_with_closure_policy`. There is no request-time
installation or current-profile eligibility inference. An installation with a
higher revision fences previous instances. The policy is only a qualification
allowlist: physical worker suppression and host proof production remain separate
unimplemented adoption/qualification prerequisites.

The owner uses the versioned aq-admission 0.1.8 candidate. Frozen 0.1.7 request
bytes are unchanged. Both suppression and actual-host-ECHILD variants retain
original logical node/boot, grant and launch identity. A pre-start stopped host
may have no pane, no root status and zero reaps; these are not fabricated.
Suppression must name its prior durable local journal and terminal checkpoint.
The qualified worker must persist launch suppression before any launch hook,
or persist driver intent before its hook and later supply the exact owned-host
stop proof. Structural decoding cannot attest those opaque local obligations.

Each request takes a five-second total deadline, including pool acquisition,
with 500ms SQL lock and two-second statement limits. The single SERIALIZABLE
transaction takes the installed policy row FOR SHARE before current enrolled
node/credential, exact attempt and hot authority locks. It verifies immutable
original initial-grant metadata, actual Prepared journal checkpoint, original
registration link, no actual Started and the independent acknowledged renewal
counter. The terminal revision must exceed both known checkpoint and renewal
counter. Renewal supplies no physical digest; a request hash is never substituted
for one. Historical Started may predate a later renewal counter; the two facts
remain separate rather than rewinding the counter or rejecting that history.

No filesystem, Graph, Git or provider I/O occurs under these locks. Cleanup
reporting does not require an unexpired execution permit or active launch
eligibility, but does require current node authentication and exact retained
historical provenance. Missing original boot provenance refuses. Registration
history records semantic identity plus runtime/qualification; attempt links
prevent ambiguous same-boot reuse. Exact receipt replay checks captured context
against the actual original owners under current policy/auth locks. It returns
the original acknowledgement without retiming or renewing permission, including
when the currently installed verifier list no longer admits new closures for
that historical tuple.

Before inserting the immutable receipt, the same transaction revokes material
leases and launch credential permission, changes hot authority to revoked
Cancelling, and preserves its original grant/owner/deadline, physical Unknown
stop and retention/effect facts. Failure rolls back all these writes. Strict
launch-open checks on initial Grant, Started and Renew prohibit fresh authority
or fresh replay delivery after closure. SQL triggers also reject reopening the
hot authority, new Started and restored launch credentials. Task/source/effect
settlement remains a separate required owner.

The closure HTTP route shares the reserved Control request class and dedicated
control SQL view; human/default routers do not expose it. Retained histories
are finite. Registration metadata caps: 4096/64 MiB globally and 1024/16 MiB per
node. Attempt links: 4096 globally/256 per node, with exactly 64 logical UUID
payload bytes each. Closure caps: 4096/64 MiB globally and 256/4 MiB per node.
Each closure retains at most 32 KiB request, 64 KiB captured context, 16 KiB
receipt, 8 KiB binding JSONB text and 4 KiB verifier JSONB text: 124 KiB variable
payload. Quotas count the actual bytea/JSONB text representation consistently.
Fixed SQL row/index overhead and PostgreSQL memory are separate; these are not
physical disk/RSS guarantees. Exhaustion refuses admission without deleting
unresolved history. Quota scans are finite but history-sensitive; no throughput
or long-lived operational envelope is claimed.

The existing prepare/renew receipt table still relies on its append-only
application owner. This slice adds no generic SQL UPDATE/DELETE guard to that
older table. Database administrators and direct mutations using the daemon's SQL
privilege remain a trusted operator boundary. New registration/link/closure
history tables do enforce immutable UPDATE/DELETE guards.

The frozen candidate passed nine actual SQL/HTTP owner cases and three schema
cases, including the cross-node quota wait race; strict all-target/all-feature
server Clippy and workspace formatting also passed. Tests use actual owner
publication and PostgreSQL, but synthetic runtime/proof qualifications. They do
not qualify a physical worker closure producer or activate the production
owner. Exact source/evidence attribution is retained separately.
