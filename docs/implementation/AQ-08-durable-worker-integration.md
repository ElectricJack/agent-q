# Durable worker grant integration probe

The independent worker process now transfers actual prepared input owners into
its durable initial-grant owner, fsyncs the received receipt, and checks current
input ownership before reporting success. The daemon probe additionally verifies
format5 disk history, exact request and delivery bytes, canonical record digest,
advanced journal revision, and conservative process-local clock bounds. Serialized
history is inspected as evidence; it never reconstructs live permission.

At daemon `82d386f` / worker `30e182f`, the actual HTTP/PostgreSQL/separate Git
process run **refused during artifact download** (0 passed, 1 failed, 3.65 seconds).
Only720ms remained at Accept under the concurrent build/test workload. Central
issued-grant history was empty. The durable-grant assertion path was not reached.
Original clocks were unchanged; no retry or timestamp adjustment was made.
The older overlap-worker success remains separately attributed to format4.

The pinned release worker binary is
`f7a5b2be22b1c82fe5398960bb158460eb59184027545a5f890995c37a51bd1b`.
Its two smoke-binary tests and strict binary lint pass. The durable grant owner's
independent [component qualification](../../../aq-worker/docs/implementation/WRK-08-durable-initial-grant-qualification.md)
records164 package tests and two actual-Git checks; those are not a successful
combined daemon-process run. [Exact report and retained logs](AQ-08-durable-worker-integration.json)
record the negative result and source pins.

Cold preparation remains blocked on the separate accepted-preparation owner and
worker adoption. No actual harness, provider, Started/result/release, or optional
native runtime activation is qualified here.
