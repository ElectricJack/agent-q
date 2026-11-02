# Actual B1 to combined review and task completion helper

Source-only test support in `tests/support/native_result_settlement.rs`. No
build, service, provider, database, review answer or Graph mutation ran merely
because this helper was authored. Runner owns all processes, actual AQ state,
worker private directory, live Graph/Git owners and cleanup.

`StableIds::capture` creates distinct question/answer/evaluation IDs once and a
five-minute question expiry. This is a new human-question timeout, never an
extension of execution, preparation, stop, source or other original deadlines.
Retain these IDs/request across response loss; do not mint another vote or
restamp expiry to recover an existing question.

`prepare_question` borrows the actual AppState, neutral reviews Service and held
central Git test owner. Inputs are the current bound fixture Owner identity,
actual recorded B1 report/import operation, borrowed same-Env PostgreSQL pool,
ORIGINAL task revision, actual finite
peer validator output and original worker-local binding, and exact bytes read
with Runner's retained PrivateState owner (openat2 BENEATH/NO_SYMLINKS/NO_XDEV).
The supplied member name is `gate04-proof.txt`, relative to that actual worktree;
the helper never opens an outer path, writes a file or invents a proof boolean.

The helper reloads actual B1/stopped/source owners, refuses legacy fallback,
cross-checks original tool/report/launch/outcome identities and fixed conclusion,
then asks the existing central Git owner for the retained checkpoint tree. The
actual complete bounded tree must contain exactly the fixed file and Git blob
OID for the independently checked31-byte literal. Source checkpoint/repository
come from actual central confirmation, never a caller-invented receipt. Evidence
ArtifactRefs hash the observations; they are NOT retention or check receipts.
No source/status/Graph proof row is directly seeded or rewritten by this helper.
Mutations occur only through the actual protected owner methods.

One NEW explicit combined question is prepared/admitted/inspected through real
owners. `human_handoff` returns its exact subject/request for a later human
interface and asserts that no choice has been made. The raw local binding nonce
and private credentials are not returned in that handoff.

`finish_fixture_choice(ApproveAfterExactProof)` is an explicit test-controller
SIMULATED Owner answer, labelled as such in its comment and returned evidence.
It is not a real human approval or agent permission. After that answer, actual
finite eligibility/release and completion owners still reload their own current
source/ledger/policy/Graph evidence and can refuse. No helper directly writes
Graph or manufactures release/receipt/outbox rows.

`settle_recorded_answer` resumes from an already protected exact Approve without
creating another answer. It does not infer whether that earlier answer came from
a person or simulated fixture. Both paths retain actual evaluation/completion
results; possible/unknown/conflicting completion remains held, never a successful
task assertion. A returned receipt is checked against the original CAS/logical
revision+1, while Runner independently verifies real Graph current state/receipt.

The tool cmp/file assertion is not a general required-check executor. The
finite local provider peer is not a paid model. Source/code retention, observed
execution, explicit review, effect disposition and Graph task completion remain
distinct facts. General outside-world isolation and a real human lifecycle are
not established by a simulated fixture answer.
