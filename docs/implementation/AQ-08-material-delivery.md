# Private preparation material delivery

`POST /v1/worker/profiled/material` returns private provider material only for an
existing, accepted native preparation. It consumes the published
`aq-admission 0.1.3` `MaterialFetch` request. The endpoint does not issue leases,
create installation receipts, call providers, or grant permission to launch.

The supported initial profile has one `model-authentication` lease, with equal
lease and preparation expiry. The original accepted `PreparationPlan` must be
retained under the operation referenced by the hot preparation state. Its exact
binding, logical guardian/launch/reservation owner and lease must match. A lease
must be `admitted`, attached to that existing attempt through
`profiled_launch_materials`, and match the node, boot, offer, selected credential
revision, provider, account and endpoint. Pending leases, installed-credential
fallback, missing preparation records and multiple-material plans are refused.
This narrower profile does not redefine the published contract's larger bounds.

The handler reads at most 64 KiB from the explicit configured private file outside
SQL locks. Two blocking readers are admitted at a time; their slots remain held
until the reader finishes even if its two-second request wait expires. These
reads select candidate bytes only. The final transaction locks the current node,
installed native configuration, attempt/hot authority, intelligence registry and
lease in that order. It rechecks credential rotation, current qualified delivery
capability, the held task claim, preparation status and cancellation/revocation.
The lease must still be linked and unchanged. Final time is sampled after the
last lock; original absolute expiry is never extended by retries or lock waits.

Success is `application/octet-stream`, `Cache-Control: no-store`, with exact
material bytes as the body. The `aq-material-delivery` header is URL-safe base64
without padding of a JSON `MaterialDelivery` containing the lease and freshly
computed `DeliveryClock`. The encoded header is limited to 4096 bytes. Metadata
that exceeds this bound is refused before delivery, even if valid under the
larger standalone contract. Requests are limited to 16 KiB. The worker verifies
identity, digest, size and expiry using `MaterialDelivery::verify_received`,
charging the entire request/response interval against the remaining TTL. A
response arriving after that local deadline cannot authorize installation.

Only the machine listener mounts this route. It uses bulk admission and ordinary
SQL connections, preserving execution-control capacity. Repeated fetches are
reads; they return a shorter remaining TTL and create no durable secret copy or
receipt. Failure bodies contain no private file paths or secret bytes. Private
reader buffers are zeroized; that does not promise erasure of HTTP, kernel or
remote client buffers. No response is evidence that installation succeeded.

The tests exercise the production router and disposable PostgreSQL with synthetic
material and runtime qualification. They verify exact bytes/headers, shrinking
TTL, expiry, changed/missing/revoked material, scope and registry mismatch,
listener/body bounds, cancellation and missing preparation, credential rotation
during an observed node-lock wait, and lease-lock waiting across expiry. They
also verify unchanged hot authority and no installation receipt. A pure boundary
test shows that valid large public metadata can exceed the explicit HTTP profile.
These checks do not qualify real provider credentials or a worker installation
adapter. Native offer/accept/prepare/grant issuance remains separately tracked.
