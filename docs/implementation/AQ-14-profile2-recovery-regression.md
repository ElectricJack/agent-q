# Catalog2 candidate: confirmed Accepted recovery regression

This is a reproducible incomplete candidate, not green integration evidence.
Frozen runner source a463e59 (runner/postgres1.1, host1.1) remains byte unchanged.
Profile1's registry/fixtures and scope validator remain fixed. Catalog2 is
explicitly configured on a new namespace ID; there is no in-place profile switch
or generation migration. A mismatch refuses adoption. Both catalogs use the
existing exact source closure until a new runner publication is qualified.

The candidate adds finite decision.request and operation.get contracts, explicit
capabilities and a fixed owner completion producer. operation.get is read-only
Effect::Observation but conservatively Recovery::Reconcilable, because frozen
host1.1 admits only reconcilable registrations. It reads a retained known decision
handle, never an arbitrary service/URL, write retry, or caller verdict. Pure real
compiler tests prove old catalog refusal and new async completion/reconciliation
registration. No host1.1 source or publication version is silently mutated.

The actual question admission, bridge effect receipt and current runtime/grant
checks join the same SQL transaction. Review budget is locked before bridge
namespace/grant; the new grant lookup defers user-row locking until common result/
source/subject/decision locks. Grant expiry is checked after waits. Source/Git
reads occur before write locks. Requester is the actual configured namespace
owner user; playbooks never call human resolve or invent a service user approval.

Human finality and its protected owner outbox remain atomic. A bounded two-event
copy verifies owner request/subject/receipt/payload, copies to the existing
protected engine outbox, and ACKs owner-copy in that transaction. Engine event
acceptance/fanout ACK is separate, with exact producer correlation and retained
replay. Cancellation is a separate keyed control operation and cannot retract a
human answer. Full implementation qualification remains pending below.

The new actual SQL/protected-engine regression
`protected_engine_question_waits_for_real_owner_and_exact_completion_event`
commits a real question, then injects a lost Host callback response (test-support
only, not a fake business receipt). Engine records Unknown and blocks its run.
Explicit recovery queries the exact existing owner receipt and obtains Accepted
with the same immutable handle; the question is not recreated and no new Git
source read occurs. Runner apply_result nevertheless only clears Blocked for
IntentStatus::Completed. The confirmed Accepted intent therefore leaves the run
Blocked instead of taking accepted→notify/join. A later final owner completion
can unblock it, but accepted-branch notification could be skipped indefinitely.

The regression intentionally fails its Waiting assertion under runner1.1. This
must be fixed by a separately versioned runner/source publication and dependent
pins. No manual run-unblock, altered receipt, test ignore, or claimed recovery
closure is used. The normal profile2 path previously reached real owner approval
and completed; its first fixture expectation incorrectly counted asynchronous
completion as a new trigger destination and was corrected to zero. Trigger fanout
count and existing asynchronous wait completion are different.

The candidate's source/Git inputs use synthetic known-native launch/retention
fixtures and a local public-Git protocol peer. Actual PG/engine behavior is real;
selected native CLI/current public-Git retention qualification is not claimed.
Migration026 is still unregistered and tests install its tables explicitly without
changing state24. Default production construction still enables no review routes
or playbook profile2 runtime. PB04's25ms budget remains open.
