# Refresh terminal owner integration

The refresh terminal owner uses the qualified `aq-phase-terminal-protocol 0.1.1`
`refresh_v1` API over actual retained `aq-admission 0.1.10` refresh records. The
frozen terminal API/definitions remain separate. No old proposal, accepted
receipt, execution receipt or Context is converted into a new nominal DTO.

The two nominal modules instantiate the same actual Started, paired Stopped,
no-Started closure and immutable history algorithms. Their private Contexts hold
the actual typed proposal/accept/prepare/execution chain, runtime registration
and retained Prepared checkpoint. Initial authority has explicit refresh
no-launch/execution variants; legacy readers still refuse both phase profiles.
No-launch history cannot construct a grant view or invocation token.

Both profiles share the existing bounded work/control/history lanes and the
same SQL table quotas/reserved terminal storage. The exact new qualification
prefix is `aq.phase-terminal-refresh-qualification/`; lookup is the exact
runtime/qualification plus exact terminal contract. Historical cleanup verifier
definitions remain usable after launch eligibility retires, without restoring
current execution permission. New first-Started checks retain current node,
principal, project, catalog, runtime, original grant and deadline fences.

Ingress uses the unchanged authenticated listener/URIs/header 1/body bounds.
The complete contract artifact chooses the nominal decoder, then the original
bytes go through that strict decoder. Duplicate/unknown fields, mixed features,
unknown contracts and cross-profile origins cannot fall back. Both Prepare arms
independently require their own paired terminal qualification: mounting the
shared URI for refresh cannot expose the frozen issuer without its old terminal
qualification. Defaults install no new eligibility.

SQL 33's schema 1/protocol `aq.phase-terminal.v1` and origin `phase_execution_v2`
remain truthful family markers. New exact contract/features/hash domains and
typed Context distinguish the profile. Current-store fixtures use actual Store
startup and its real registered format; this change adds no DDL or metadata
restamp. Existing historical 32→33 fixture behavior remains explicit.

The blocking private material-read budget is also shared across nominal Accept
owners. Its two permits remain owned by actual blocking jobs after HTTP waiter
cancellation; the second profile cannot silently double unfinished read capacity.

## Qualification scope and evidence

All-feature library typecheck passed 18.97s. Six actual current-store HTTP/SQL
cases passed 6/6 in 16.56s after 46.93s compilation, using default thread stacks:

- actual refresh Prepare delivery reserves current terminal storage;
- exact Started and paired Stopped ACK bytes replay without new mutation;
- current authenticated history remains historical after original expiry;
- duplicate/unknown/mixed-profile requests preserve authority unchanged;
- retired/expired runtime cleanup retains task/source/effect holds;
- no-Started closure revokes permission, retains holds and replays exact ACK.

The stronger seventh mixed-profile case passed 2.30s (3.29s test rebuild). It uses
an actual frozen accepted attempt with both execution qualifications and only
refresh terminal qualification. Public HTTP refuses 503/no grant; direct trusted
frozen library issuance then succeeds. This proves the blocked request was
otherwise eligible, rather than a malformed or cross-profile fixture. Its
acceptance clock is controlled only at the genuine new decision boundary; old
library execution uses the real clock.

Selected frozen regressions passed: original Started ACK/replay after expiry
(2.77s), cancelled/expired paired-stop ACK after retirement (3.17s), lost-response
history projection (2.72s), pre-Started host-stop closure (2.41s), and all 3 original
HTTP version/type/auth/body/control-pool cases (1.96s). The shared presence setup
helper is now loaded once by `common::presence`, independent of nominal phase
profile; no duplicate fixture static/module is introduced by the mixed case.

These tests use actual authenticated owners, current SQL rows, wire decoders,
canonical envelope/digest joins and immutable ACKs. Host/stop proof DTOs are
explicitly synthetic. No physical process, kernel ECHILD, worker binary,
provider/model, live canonical state, or production eligibility is qualified by
this evidence. Source review plus the explicitly pinned worker 0.4.0/FORMAT11
real-clock static Watch→UploadAck→tail run remain separate gates.

Selected strict Clippy passed for the all-feature library and all six touched
terminal test targets with warnings denied (3.79s). Default library check passed
27.25s. Formatting and diff checks passed. There were no runtime/source changes
between these checks and the freeze; the final shared-presence cleanup is an
alias of the same unmodified profile-independent fixture implementation.
