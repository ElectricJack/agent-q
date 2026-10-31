# Explicit AQ host catalog2

This catalog adds only decision.request and finite operation.get to catalog1's
message.send. It is not the standalone CLI's synthetic twelve-command host.
The descriptor pins exact qualified source d3bd349 and package definitions;
model/compiler1.0 and frozen review/session protocol bytes remain unchanged.
Catalog1 files remain their original publication, not a claim that a new runtime
uses the old source. New namespace metadata carries the actual current runtime
source digest; old-origin namespaces are not silently adopted or migrated.

Only actual AQ owners can admit questions and produce completion events. A
playbook can request a question, never resolve it as a human or post a passed
check. Accepted, human answer, expiry, cancellation, transport ACK and task
eligibility are separate. Expected subject/recipient binding and current grants
are checked by the actual database owners; JSON shape and digest alone confer
no authority. No production default builder enables this optional catalog.

operation.get selects only a known human_decision handle in the namespace and
current task/session scope. It has read-only effect and conservative Reconcilable
recovery to fit the pinned owner registration guard. It is neither a generic
service/URL proxy nor an assertion that an old query snapshot is replayed.

The editable definitions are server playbooks/review_domain.rs. Regenerate with
`cargo run -p aq-server --example playbook_review_contract --offline`; pure
compiler/schema tests reject drift and prove catalog1 cannot authorize the new
command. Runtime qualification is separately pinned in AQ14 implementation
evidence. Native/source/check-executor integration gates remain explicit.
