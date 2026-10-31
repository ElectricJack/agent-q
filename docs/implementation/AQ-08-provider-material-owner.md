# AQ08 provider material owner

The daemon can explicitly configure `delivery_mode = "server_material_v1"` on
an intelligence credential with an absolute private `secret_file`. Without this
mode the existing installed-credential behavior remains unchanged. Caller-set
`provisionable` or `material_owner` fields are refused; loading a safe nonempty
file derives these facts. Missing files produce no material owner. The owner
artifact identifies the exact profile/revision/provider/account/endpoint and
server delivery adapter version; its descriptor contains no path, secret value,
or private fingerprint. A changed credential requires its normal immutable
revision update; the existing registry installer rejects changed private
fingerprints under the same credential revision.

Both startup loading and delivery open the configured file with no-follow and
nonblocking flags, require a regular file owned by the daemon UID with no group
or other access, and read at most65537bytes to enforce a65536byte ceiling.
Owned secret buffers are zeroized when dropped. Delivery compares the exact
loaded fingerprint, lease digest/size, node, credential profile/revision and
logical provider/account/endpoint. Errors contain neither paths nor secrets.
Zeroization does not cover HTTP-stack copies or the remote receiving process.

This is a private file-owner primitive, not a delivery authorization route.
The eventual material endpoint must read outside database locks and then lock
and recheck current machine credentials, registry revision/private fingerprint,
exact lease and launch binding, assignment/boot and expiry before returning
private bytes. A preparation receipt must verify delivery before any usable
execution grant; readiness never means a key is installed. No provider network
call is made to infer account identity or availability. Actual runtime offer,
prepare, delivery and worker installation remain separate integration gates.

## Native inventory preparation facts

The existing inventory route now authenticates with the opaque current-node
context and rechecks the credential under the node lock. For an actually admitted
native runtime, its exact configured delivery adapter can enable a fresh bounded
file check before any SQL locks. Up to four blocking file-owner observations run
concurrently; each retains its admission permit until the file read finishes,
including after the two-second request wait expires. No model/provider call is
made. The final transaction rechecks the admitted runtime, full installed
configuration anchor, registry revision, boot and observation age after lock and
quota waits. Observation time is retained rather than refreshed after waiting.

Derived credential bindings have `available=false` and exact node/boot/adapter
preparation provenance. Missing, unsafe or changed files yield no preparation
fact. Workers cannot submit delivery capabilities or preparation facts in the
inventory request, and `available=true` is refused for credentials configured for
server delivery until the real verified material-receipt owner exists. Existing
installed-only mode retains its original report behavior. These snapshots can
make a cold candidate eligible for preparation; they cannot issue a grant.
