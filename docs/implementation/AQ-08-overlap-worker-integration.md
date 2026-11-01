# Independent worker integration with bounded read overlap

Daemon source `6d4b178` (schema28, renewal and optional closure-owner code) and
worker source `a39e990` pass the actual independent-process preparation test in
3.17seconds, including fixture setup. The
[manifest](AQ-08-overlap-worker-integration.json) pins the exact release worker,
Git binary and raw result. The server test uses an isolated build directory.

This passes actual central publication, outbound HTTP, durable input requests,
private installation, separate central/worker Git owners, worktree hydration,
Prepare and a locally usable initial grant. It asserts actual worktree bytes,
retained material receipt equality, held task and no Started. No host launches.
Harness/runtime/Graph/provider qualification remain explicit synthetic fixtures.

The bounded two-read change uses existing HTTP slots and does not change any
original clock. Earlier schema28 refusal during hydration remains retained in
AQ-08-schema28-renewal-integration; the passing tiny fixture is not a latency
benchmark or cold-preparation qualification. The separately versioned accepted
preparation and durable received-Grant owners remain required.
