# AQ-08 refresh preparation consumer increment

This first consumer slice adds the separately configured
`accepted_refresh` proposal/accept/retrieval owner. Execution issuance and refresh
terminal owner dispatch are not yet connected; this is not activation.

The exact contract selects a closed typed parser, which then decodes the original
wire bytes. A header probe never replaces those bytes with a Value serialization:
duplicate and unknown fields still refuse. Proposal handling retains the existing
Json extractor's media-type/syntax behavior. Unknown contracts, mixed negotiation
features and mid-attempt profile substitution have no fallback.

Refresh policy definitions use `aq.accepted-preparation-refresh-policy/` and
the exact admission0.1.10 profile artifact. The existing policy namespace remains
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
