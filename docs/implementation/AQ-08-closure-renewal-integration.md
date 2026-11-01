# Closure and renewal integration

Source `fbe90e9` passes44 selected checks:10 closure-owner,31 native initial-grant/
renewal and three schema28 checks, with zero ignores. Both actual-Git grant tests
are enabled. Affected all-feature strict Clippy and workspace formatting pass.
The separate known cold-preparation process probe is explicitly filtered; its
failure is retained in the previous integration evidence. Exact hashes and
negative runs are in the [manifest](AQ-08-closure-renewal-integration.json).

The new combined test first acknowledges a renewal, rejects older suppression,
then records a later qualified terminal checkpoint. Exact old-renew replay,
new renewal and initial-grant replay all refuse afterward. Historical closure
replays unchanged; the original grant and task/source holds remain for separate
settlement. A renewal counter is never misrepresented as a physical journal hash.

This profile uses two test threads on the shared development host. The earlier
higher-concurrency run passed nine closure cases but expired in initial Grant
fixture setup before exercising the capacity case. No deadline or production
capacity was widened to obtain the bounded-profile pass. The first root test
compile also used a private module path and wrong response-field accessor;
those test-only mistakes and their correction remain visible in the logs.

The server owner is enabled only through explicit installed historical verifier
policy injection; the default worker router still has no closure endpoint.
Synthetic qualified suppression/host records are not physical worker evidence.
The worker producer, native host driver, settlement and operational load profile
remain open. The live preview stays at independently qualified `8f1adcc`.
