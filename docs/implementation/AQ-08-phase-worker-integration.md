# Real worker accepted-preparation integration

The independent worker process passes its first schema30 daemon integration at
server `4fa0dce` / worker `cf943ff`: one test,4.27seconds. It uses actual HTTP,
PostgreSQL, private journal files and separate actual aq-git services. The worker
reserves capacity, obtains the exact proposal, atomically records its phase Accept
intent, receives and fsyncs the no-launch lease, waits through the original delivery
window, and replays the same operation with a fresh transmission nonce.

The original proposal expired at1793503725485ms. Actual replay server time was
1793503726021ms,536ms later; the independent accepted lease still expired at
1793503753849ms. Original clocks were not widened or retimed. The worker's format6
journal remains at revision4 after replay with identical first received history.
The test compares actual worker disk records with daemon request/receipt rows,
checks local conservative caps, and proves the daemon remains Preparing/held with
no Grant, Started, legacy Accept or installed-material receipt.

[Source pins, binary hashes and retained logs](AQ-08-phase-worker-integration.json)
include the two smoke-binary tests and lint checks. The earlier legacy preparation
refusal is retained separately and is not reclassified as a pass. The phase probe
uses synthetic runtime/harness/provider/Graph qualifications. Actual Git service
use here covers publication and reservation preflight, not phase worker hydration.

Worker artifact/material/source adoption is next. Fresh execution confirmation,
host launch, continuation, settlement and fleet-load qualification remain open;
this integration does not activate a native runtime or widen the old protocol.
