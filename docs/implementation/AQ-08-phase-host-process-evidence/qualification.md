# Actual daemon, Git and tmux phase execution checkpoint

Server test source `75a460e` and worker smoke source `b799930` (integrated as
`2a818f4`) join the previously separate format9 physical host and schema32 issuer.
The new mode is opt-in and compiled only with test support. It changes no
production route, runtime capability, lease duration or provider configuration.

The actual process test passes in **6.99 seconds**. An independent worker registers,
receives a real phase authorization over daemon HTTP/PostgreSQL, fetches38 artifacts
and one material, and hydrates a real workspace through pinned aq-git/Git. It retains
execution at journal revision27, launches the explicitly installed synthetic ELF
through the production host helper and actual tmux3.4, observes positive Running,
and persists kernel-confirmed ECHILD shutdown at revision34. The test verifies
exact grant/guardian history, the startup payload and workspace directory. The
synthetic program gets a bounded500ms window to write its marker; this does not
extend execution permission. Stop runs before result inspection/serialization,
including ambiguous launch outcomes. Existing resource/task holds remain.

With the same pinned worker, the existing execution-only test passes in5.64seconds
and the input/expired-proposal test passes in6.97seconds. Both existing binary
unit tests pass. Strict Clippy passes for the changed server test target and worker
binary; formatting passes. This is targeted validation, not a full-suite rerun.
The retained earlier server compile failure was a missing test-module path,
corrected before the successful test binary was frozen.

Physical Started/Stopped are local history here: no remote phase lifecycle ACK,
renewal, task completion, settlement or source release is claimed. The registry,
provider, runtime/harness qualification and Graph evidence remain synthetic.
No real provider credentials or external provider calls are used. These timings
include fixture work and are not service latency or fleet-capacity measurements.
The dashboard preview remains on its separately verified schema32 binary.

[Exact sources, binary pins, measurements and logs](qualification.json) are retained
alongside this report. The server test executable was copied after compilation and
invoked directly, avoiding changes to its build target during qualification.
