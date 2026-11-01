# Composed taskless selector and empty Playbooks owner

The default state-37 server at source `511ee995ac55dd6345590732705b5a3bfde5130d`
combines the independently reviewed `aq-selection` 0.1.4 with the reviewed explicit
empty-deployment Playbooks startup path. No task-selection wire or frozen loaded
definitions profile was changed. The new central selector returns actual linked
decision bytes and unavailable accounting; it supplies no durable owner or
execution authority.

The composed gate passed 38 selector tests, including the captured 23-vector legacy
task corpus, and all 15 actual Playbooks activation tests. Server library/all-test
compilation and the default binary build also passed. The separate extracted
component closure passed 75 tests with strict lint and formatting checks.

The immutable default binary is `/tmp/aq-composed-central-empty-511ee99/aq-server`,
SHA-256 `17698c625675b7a994b0a56bbd37737becbf039380093ae9dd32fe1bdcd27b25`.
Its runtime source stamp is `511ee99`; it has no test-support feature. Exact counts,
timings, normalized terminal logs and checksums are retained in
[the qualification record](evidence/composed-central-empty-511ee99/qualification.json).
Only trailing blank lines were removed from the copied terminal logs.

At this checkpoint the canonical preview still uses its state-35 backend and the
qualified `624707540cc6566158c3721e4733f69934c8a1670c4ffb972afb7d469fb8e663`
dashboard. Its optional-owner cutover is a separate operation. A real Unix-socket
owner and a new empty namespace were qualified independently; an empty startup
deployment list does not uninstall retained definitions, grant execution, or invent
workflow activity. See the [staging plan](canonical-playbooks-uds-staging-plan.md).

The worker main branch separately contains the qualified standalone Responses
transport and finite prepared-capsule composition. Those tests use an isolated
static guest and local synthetic upstream; they do not qualify Codex, provider
availability, production SourcePin/permit joins or suspend/wake. AQ-17, API-05 and
WRK-17 remain in progress, with their new evidence linked in the implementation DAG.
