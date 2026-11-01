# Granted launch closure before centrally recorded Started

Status: proposed next integration slice; no wire contract, migration or runtime
capability is implemented by this document. The existing admission0.1.7 bytes and
migration27 remain frozen. Both daemon and worker must qualify the new boundary
before claiming complete native restart recovery.

## Problem and existing owners

A worker can receive a grant, then fail before Started reaches the daemon. It may
have never called the host, stopped inside the pre-exec baseline gate, or run the
harness while its Started request was lost. Absence of central Started therefore
does not prove absence of execution or source changes.

`profiled_execution::no_grant` and the SQL NeverGranted gate correctly reject any
issued grant. `aq_admission::stopped::Request::validate_after` requires the exact
previous Started record; it intentionally cannot describe this gap. The native
journal currently reopens only for historical inspection. Do not relabel these
attempts as NeverGranted, invent Started, or free their resources on timeout.

## Two distinct local evidence paths

**Suppressed before any launch intent.** The qualified native journal acquires
the original root's exclusive ownership, validates the supported complete
published snapshot and original reservation/Prepare/grant, and finds no durable
host-launch intent. It writes and fsyncs a terminal launch-suppression record
before returning an opaque closure receipt. No later code path may append a
launch intent for that attempt. The record includes original logical and kernel
boots, reservation/guardian/launch IDs, immutable grant identity, preceding
journal digest/revision and suppression operation/digest/revision. A missing,
corrupt, unsupported or ambiguous snapshot supplies no such receipt. Existing
historical getters alone cannot manufacture it.

This proof relies on the qualified driver's invariant that a durable launch
intent must precede every host side effect. Implement and crash-test that
invariant before enabling suppression. It is not cryptographic kernel
attestation against a malicious same-UID worker or operator replacing its root.
Recovery may close old ownership but may not renew it, relabel its boot, restore
agent credentials or advertise the held slot as available.

The existing host calls the trusted adapter's `prepare` hook before its own
launch-intent callback. That callback alone cannot establish this stronger
driver invariant: arbitrary adapter Rust could create an external writer. The
native driver must persist its intent before invoking any host/adapter startup
hook that can create effects, or separately qualify those earlier hooks as
non-spawning and non-writing. Prefer the earlier driver intent; it safely turns
an interrupted hook into an unknown launch requiring recovery.

**Exact host physically stopped.** Once a launch intent exists, journal absence
of Started is insufficient. Recover the exact original host identity and obtain
the real opaque `aq_session_host::StoppedProof` after the guardian's ECHILD
boundary. Retain its exact intent/configuration digest, kernel boot, PID namespace,
process birth identities, permit sequence/deadline and final observation. A new
contract must model the actual pre-start host shape, including a legitimately
absent pane/root status, without inventing placeholder process identities.
The host's real stopped-with-no-child branch can also have zero reaped children;
do not copy the post-Started contract's positive-count requirement into this path.
Never infer this proof from a vanished socket/PID, host timeout, missing files,
expired grant or a worker-supplied `stopped=true` field. An unrecoverable launch
intent remains unknown and held.

The second path means that the original host has no remaining owned writers; it
does not assert that the harness never ran. Source changes, tool effects and
accounting may still need reconciliation.

## Central admission and durable result

Publish an additive, separately versioned closure request/receipt in the public
admission contract and independently qualify its extracted package. Use the
existing outbound authenticated machine listener; no worker listener/tunnel or
database replica is introduced. Bound the first request at32KiB, one attempt and
one evidence variant, with a stable operation ID and exact canonical digest.
Private bytes and arbitrary paths are never part of the request.

The central owner locks current enrolled-node authorization, exact attempt/hot
authority, sealed launch and original grant/preparation/registration records.
Validate the original node/boot, full binding, runtime/qualification,
reservation/guardian/launch IDs and monotonic journal revision. A process restart
does not change these historical identities. Current machine authentication is
required; launch eligibility expiring or its configuration being retired is not
itself a reason to discard valid historical cleanup evidence. Define historical
qualification lookup explicitly rather than silently reusing an active-launch
eligibility helper.

Admission requires an issued grant and no previously admitted Started. Insert
the immutable closure receipt and terminal launch fence together. Revoke further
execution/renewal and credential issuance, while preserving the original grant,
hard deadline and all historical receipts. Same operation/bytes returns the
original receipt; changed bytes or contradictory later evidence refuse.
Started, renewal and closure must serialize on the same authority locks: if
Started wins, use the existing post-Started stopped owner; if closure wins, no
new Started or grant authority can be admitted. A genuinely uncertain outcome
must be inspected through exact replay, not retried under a replacement ID.

Closure alone clears **no task claim, workspace, source or effect hold**. The
receipt records `launch_closed` with either durable suppression or physical-stop
provenance. Independent source/effect settlement must account for possible writes
and in-flight graph/tool/message effects before the release owner can free the
claim and authorize local cleanup. Neither variant is task success or a passing
check. A missing Started receipt does not erase usage already observed.

Persist the worker closure request before sending it, then the returned central
receipt before any later settlement/release operation. Lost replies and process
restarts retain original IDs and holds. A later new runtime boot registers only
after the old attempts have reached actual settlement and release.

## Implementation and qualification order

1. Specify exact suppression and pre-start host-proof types against real journal
   and host output. Qualify the public contract independently; do not extend the
   frozen post-Started proof with fabricated optional/default fields.
2. Implement the native journal's terminal suppression and host-stop recovery
   owners. Prove no physical effect can precede an acknowledged launch intent.
   Test interrupted suppression writes, stale roots, old kernel boots, missing
   host state and unknown launch/stop replies. Unsupported history remains held.
3. Add an ordered migration after the current registered head, terminal hot-state
   and journal transitions, immutable receipt storage and authenticated route.
   Keep existing NeverGranted/post-Started owners byte-compatible. Test both
   orders of the Started/closure race, revoked credentials, retired launch
   configuration, changed operations and expired grants without allowing renewals.
4. Connect source/effect settlement and explicit release. Exercise actual tmux
   pre-exec stop, actual harness exit with a deliberately lost Started reply,
   persisted no-host suppression, and worker restart. Prove source changes and
   unknown effects cannot disappear merely because central Started is absent.

The first implementation may support only one evidence variant, provided the
other remains visibly unknown and held. Complete native recovery and the real
execution product gate require both supported lifecycle paths or an explicitly
qualified narrower runtime profile. Component tests do not establish arbitrary
power-loss durability or a live-provider execution result.
