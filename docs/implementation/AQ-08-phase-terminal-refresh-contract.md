# AQ-08 terminal registry-refresh contract

Source `b4503bd` adds the separately negotiated terminal bridge in
`aq-phase-terminal-protocol 0.1.1`, depending exactly on admission0.1.10. The
root API remains the frozen0.1.0 profile; its artifact/schema/manifest/complete
fixtures/negative vectors are unchanged. One private structural algorithm
instantiates both typed profiles without converting a refresh receipt into a
frozen DTO. Independent source review accepted this increment; the immutable
four-archive package closure passed separately from consumer adoption.

`refresh_v1::Context` borrows the actual typed refresh proposal, acceptance,
accepted receipt, Prepare, execution receipt, admitted runtime references and
retained server Prepared checkpoint. It validates all four registry floors in
that actual refresh history. The emitted origin contains the actual new accepted
and execution digests. New requests, ACKs, closure and history/readback use new
domains and the exact `aq.phase-terminal.registry-refresh-v1.contract` artifact.
Required ordered features add `registry_observation_continuity_v1`; unknown,
duplicate, missing, reordered or cross-profile negotiation refuses.

Family protocol/header1 remain `aq.phase-terminal.v1`, envelope schema1 and
origin `phase_execution_v2`, matching the frozen SQL33 family guard. These labels
do not equate the new exact artifact with the frozen artifact. `Profile` dispatches
only exact known terminal/preparation artifacts; a parser/configuration selection
is never an authentication, permission, host proof or runtime qualification.

All27 pure tests passed:12 existing contracts,1 exact frozen artifact and14 refresh
cases. Both schema sets passed16 positive,4 shape-negative and4 relational-negative
vectors each. Strict all-target/all-feature Clippy, deterministic export, formatting
and diff checks passed. An initial new artifact test compared struct field order
against the published Value export order; its corrected comparison uses the same
Value export representation. The actual legacy artifact and digest never changed.
The finite checks and actual hashes are recorded in
[qualification.json](evidence/terminal-refresh-b4503bd/qualification.json).

The new immutable archive SHA256 is
`818f2499a2d50f47db491031f50fc1e629fd9d402e224a460efea1ea5532805c`.
All28 tracked package payloads match `b4503bd` exactly, including source manifest
as `Cargo.toml.orig`; Cargo's VCS record names that actual commit. The normalized
manifest depends exactly on admission0.1.10. A fresh extraction of the four
explicit archives, with no live sibling checkout, passed27 tests, strict checks
and formatting under locked/offline resolution. Existing0.1.0/0.1.9 archives
remain unchanged. The reproducible closure runner is
`scripts/test-terminal-refresh-packages.py`.

No new SQL, consumer route, worker journal, actual process or production eligibility
was qualified. Physical stop still requires the actual existing producer and
ECHILD observation before durable Stopped/ACK. Historical readback never restores
live execution, extends a deadline, releases resources or settles a task.
