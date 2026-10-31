# AQ-08 pure contract checkpoint 0.1.1

Source `9a4c1c7` passed 18 admission tests, 20 selection tests, strict all-target
Clippy and formatting from detached clean worktree
`/tmp/aq08-contract-clean-9a4c1c7`. An independently unpacked admission archive
passed its 18 tests with only the exact pinned session/lifecycle packages patched.
The clean archive SHA256 is
`ff4ec1ba208bab1461e0e85dbd1291fe17778a4741fc6b1e1c622753132aed55`.
The JSON inventory hashes frozen `git show` bytes and retains the logs/archive.

This checkpoint adds transport envelopes, conservative replay clocks, exact
lifecycle binding with independently generated lineage vectors, durable host-stop
journal conformance, and explicit cold credential preparation provenance. The
original admission schema and fixtures remain byte-identical; installed-only
selection serialization and outcome are checked against the old golden fixture.

These are contracts and pure decisions. They install no admission or material
handler and qualify no real worker launch, secret delivery, provider/model call,
or remote physical-stop proof. Preparation does not grant execution; missing
keys and arbitrary worker capability claims do not establish provisionability.
The qualified host producer must supply actual durable stop evidence, and the
central source/effect owners must verify their actual receipts before release.
The bounded initial transport profile can conservatively refuse larger shared
protocol envelopes. The hot-authority migration and actual admission runtime are
separate work in progress and are excluded from this checkpoint.
