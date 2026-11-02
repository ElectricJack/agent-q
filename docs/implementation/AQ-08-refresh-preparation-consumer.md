# AQ-08 refresh preparation consumer increment

The preparation slice adds the separately configured `accepted_refresh`
proposal/accept/retrieval owner. Following increments add nominal refresh
execution, original-grant replay and paired terminal owner dispatch behind exact
profile qualifications. Defaults activate no new runtime. See
[issuer qualification](AQ-08-refresh-issuer-qualification.md) and
[terminal integration](AQ-08-refresh-terminal-consumer.md). The pinned real-worker
static output qualification remains a separate gate.

The exact contract selects a closed typed parser, which then decodes the original
wire bytes. A header probe never replaces those bytes with a Value serialization:
duplicate and unknown fields still refuse. Proposal handling retains the existing
Json extractor's media-type/syntax behavior. Unknown contracts, mixed negotiation
features and mid-attempt profile substitution have no fallback.

Refresh policy definitions use `aq.accepted-preparation-refresh-policy/` and
the exact aq-admission 0.1.10 profile artifact. The existing policy namespace remains
frozen; lookup is therefore the exact profile plus admitted runtime/qualification,
with duplicate definitions refused within that profile.

Actual preparation I/O, source/material association checks, locks, deadline
arithmetic, resource holds and quotas are shared by instantiating the existing
owner files with nominal profile types. No new/old DTO conversion invokes the
other profile's validation. The old lease constructor retains its exact values;
the refresh constructor additionally includes an actual locked registry ref.

The refresh owner checks the actual proposal registry against the retained
selection revisions and included observation revisions. It captures the actual
registry at acceptance under the current configuration/aggregate lock, requires
same-config monotonic continuity from the actual proposal ref, and records that
ref as `registry_at_accept`. Retrieval loads the actual typed refresh history and
checks the actual current registry against that accepted floor. It never remints
the original selected reference or changes either observation's time.

Source checkpoint `9e0720b` plus the following small compile fixes passed an
all-feature library typecheck (26.63s) and two pure dispatcher cases (0.03s;
library-test rebuild1m41s). These cases cover exact old/new nominal dispatch,
mixed/unknown contract refusal, duplicate/unknown field preservation and byte
bounds. No database fixture, worker, host, model or provider ran. This finite
verification does not qualify actual owner issuance, four-floor prelaunch
continuity, current SQL history, terminal ACKs or end-to-end output. Those remain
the next integration work, with original limits and frozen histories preserved.

## Shared issuance and monotonic original-grant replay

The next source increment instantiates the actual issuer with the refresh types.
It records the actual proposal, acceptance, preflight and final registry refs in
both the typed Registry fact and immutable provenance. The preflight owner read
occurs before external Graph/project/Git/private backing observations; the final
locked read must not roll back from it. Each ref has the same immutable config
identity and canonical positive revision. The exact retained selection, concrete
choice, credentials, material installation, principal, inventory compatibility
and existing policy/budget checks remain required. Initial issuance consumes the
same four-request lane and shared durable quotas as the frozen issuer.

A refresh execution replay delivers only the original still-live grant. Its
actual current registry must be at least the original issuance registry and the
actual replay preflight read. After releasing SQL locks, replay collects real
bounded external owner observations. It then reloads the same immutable issued
history, checks the current registry floor and all current health, cooldown,
source-age, principal, policy, node, material and inventory fences under locks,
and charges the final clock after every I/O/lock acquisition. Expired original
grant/deadline authority never revives. A private backing read verifies bytes
without renewing the original private retrieval lease.

The original request, receipt, Registry issue facts, provenance, digests, grant ID,
issue time, expiry, execution deadline and budget origin remain byte-identical.
Only the nonce/current server delivery clock changes; its expiry is exactly the
original grant expiry, so RTT and client clock bounding can only reduce remaining
authority. The new current observations are delivery checks, not replacement
historical issuance facts. There is no reselection, new grant, accounting reset,
lease extension or new installation record. Frozen 0.1.9 replay still takes its
original exact-registry branch with no additional observation I/O.

The refresh public execution transport stays closed until the actual paired
refresh terminal owner is installed and qualified. Library owner tests do not
advertise runtime eligibility or production activation.
