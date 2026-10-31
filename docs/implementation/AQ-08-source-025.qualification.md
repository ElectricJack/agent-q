# AQ08 native source owner/read checkpoint

Frozen source `881ee8e` adds functional migration025, the actual typed Source-role
inline observation owner, and assigned-worker version1 status/chunk retrieval.
The old Context proposal receipt remains unchanged. Published admission/session
contracts are unchanged.

37 focused SQL/router/store tests and one migration metadata unit pass. Strict
server all-target/all-feature Clippy and formatting pass. One existing real-Git
peer test is explicitly ignored; the source read HTTP fixtures use synthetic
public Git retention peers and bytes. Actual Git helper qualification is separate.

The native tests exercise exact manifest/bundle chunks, Preparing original plan,
Granted expiry and cancellation, historical receipt replay versus stale new
publication, credential rotation and offer expiry during paused Git inspection,
unknown version/fields/ranges, invalid credentials, and nofollow file refusal.
Reads create no grant, owner journal or execution receipt. Store tests apply the
normal ordered migration chain, including idempotence and upgrade cases.

This checkpoint does not complete the native scheduling/offer/accept/prepare/
grant/renew producer or qualify a real model/counting/harness combination. Current
retrieval authorization does not provide launch permission. Full combined root
workspace qualification is a separate integration gate.

Exact commands, frozen `git show` SHA256 inventory, log byte counts and hashes are
in [the qualification record](AQ-08-source-025.qualification.json). Retained logs
are in [the evidence directory](evidence/aq08-source-025-881ee8e).
