# F1a: durable central session owner; physical host blocked

Task: `steady-harbor-87.1`. Overall outcome: **blocked on `crisp-nexus-12`**.
This is a partial implementation, not supervisor execution qualification.

## Authorized scope and prerequisite

Operator message `msg-4ced906430644c58b79ae1ca9352aff3` directs this worker to
finish independent work and close blocked on the newly filed aq-worker
prerequisite. Initial findings are retained in the task comments; escalation
`msg-8ce999c57553405db80e69850214c91c` reported the contract mismatch.

At inspected aq-worker revision `d178eb78a37f857f4ac8061ea7da5bc17d2648f9`,
`aq-session-host` 0.1.5 accepts only `LaunchManifest`. The pinned v1 protocol
requires non-nil project, graph, task and attempt IDs. The AQ-17 implementation
reference explicitly requires a new subject-neutral HostPlan and forbids
placeholder task IDs or weakening v1 validation. B6a1 evidence checkpoint
`a3d9411df015055d35d8db3c4a7fa2cbb6579a70` reports no implementation delivered.
No sibling code or frozen protocol was modified.

## Independent implementation

- `crates/aq-central-authority/src/owner.rs`: dormant, trusted-process SQL API
  for prepare, start, renew, verified reports, current inspection and retained
  generation inspection. The actor lock serializes concurrent writers. Actor,
  conversation and authority IDs remain fixed; every successor increments once.
- `src/owner/history.rs`: derives the candidate MailHistoryFence from all retained
  generations, including unknown/pending/handed-off mail, original cursor/payload
  identity and cross-role operation reuse. It accepts no caller-supplied fence.
- Migration `0045_central_sessions.sql`: head and generation storage, bounded
  state bytes, checked digest, unique session IDs and immutable historical rows.
  Migration registration, readable formats and Docker state format advance to 45.
- Existing reducer checks own permit duration, next sequence, strictly later
  expiry, original deadline, boot identity, clock regression and terminal state.
  Refused late renewals still commit the expired/stopping state. Start replay
  returns the retained association and never extends its permit.
- A required `EvidenceVerifier` receives the same locked SQL transaction. No
  production verifier, HTTP route or daemon activation is supplied. In-process
  callers must verify current policy, original mailbox facts and actual physical
  evidence; structural artifact references alone cannot establish stop proof.

The owner retains up to 1024 generations, each at most 64 KiB, and bounds each
operation to four seconds (two-second database lock timeout, three-second SQL
statement timeout). Capacity exhaustion refuses; no automatic history deletion
is implemented. These are logical bounds, not a production capacity claim.

## Verification environment and commands

Source base: `10a4cd3d4f88599d65171912948b283bf9357ea8`.
Implementation plan commit: `f69409ad`. Implementation commit: `b2a05237f0673709c46cb6109533e2f3d118927e`.
Linux 6.18.33.2-microsoft-standard-WSL2, Rust/Cargo 1.96.1, PostgreSQL
17.11 (Debian) in disposable container `agent-q-test-db`.
Every Rust test/lint command uses the retained pytest adapter through `aq test`;
Cargo build jobs default to 2, debug info and incremental builds are disabled,
and Rust test threads are bounded. SQL fixtures use a fresh database per test.

The adapter in `F1a-evidence/test_cargo.py` records exact Cargo argv and writes
separate logs. Runtime SQL tests use the disposable endpoint from `scripts/test-db`.

- Initial red: `F1A_CHECK=owner DATABASE_URL=<disposable-test-db> aq test
  docs/implementation/F1a-evidence/test_cargo.py -q --timeout=1800` failed with
  unresolved import `aq_central_authority::owner`; retained in `F1a-evidence/red.txt`.
- The first SQL run exposed PostgreSQL's rejection of `convert_to` in a generated
  column (it is STABLE, not IMMUTABLE). The corrected migration stores and checks
  the digest, and the owner writes it in the same transaction. Failed-run evidence
  is retained in `F1a-evidence/migration-failure.txt`.
- The corrected owner run passed **8/8** tests, with no ignored or filtered tests:
  `F1A_CHECK=owner DATABASE_URL=<disposable-test-db> aq test
  docs/implementation/F1a-evidence/test_cargo.py -q --timeout=1800`.
  Underlying command: `cargo test --locked -p aq-server --test central_sessions
  -- --test-threads=1`. Log: `F1a-evidence/owner.txt`.
- `F1A_CHECK=model aq test docs/implementation/F1a-evidence/test_cargo.py -q
  --timeout=1800` passed **16/16** central model tests. Underlying command:
  `cargo test --locked -p aq-central-authority`. Log: `F1a-evidence/model.txt`.
- `F1A_CHECK=format aq test docs/implementation/F1a-evidence/test_cargo.py -q
  --timeout=1800` passed (`cargo fmt --check`, exit 0; silent format log).
- `F1A_CHECK=clippy aq test docs/implementation/F1a-evidence/test_cargo.py -q
  --timeout=1800` passed. Underlying command: `cargo clippy --locked
  -p aq-central-authority -p aq-server --lib --test central_sessions -- -D warnings`.
  Log: `F1a-evidence/clippy.txt`.
- `F1A_CHECK=format_version aq test docs/implementation/F1a-evidence/test_cargo.py
  -q --timeout=1800` passed the **one matched** migration-version test (188
  unrelated unit tests filtered out). Underlying command: `cargo test --locked
  -p aq-server --lib state_format_is_the_latest_migration`.
  Log: `F1a-evidence/format_version.txt`.
- `git diff --check` passed. No physical-host acceptance pass is claimed.

## Remaining work and limits

Both task deliverables remain unmet:

- `cargo test --locked --workspace supervisor_session_starts_under_central_permit`
- `cargo test --locked --workspace supervisor_restart_resumes_without_duplicate_session`

No persistence-only fixture uses either name. The SQL tests use synthetic trusted
host evidence and do not prove physical stop, live adoption, guardian expiry or
provider operation. A reopened owner reads historical data; it does not observe
or re-adopt a live process. Production CLOCK_BOOTTIME acquisition and evidence
verification belong to the missing host bridge.

After `crisp-nexus-12`, finish pinned shared host/mapper vendoring, the
`aq-supervisor-host` binary/image, central credential delivery, the production
verifier, real restart/expiry qualification and both named acceptance tests.
Historical handoff reconciliation after a successor is not exposed by this
partial owner; stale-generation reports currently refuse. No credential bytes,
mailbox dispatch, supervisor tool credential, Compose service or worker
enrollment was introduced.

Cross-family adversarial review has not been performed; it remains required
before a passing submission. The full workspace test/all-target clippy suite was
not run: prime limits this worker to focused and related-area validation.
`refs/remotes/origin/dev` is absent, so the prescribed final `git merge origin/dev`
is unavailable; no alternate default branch merge or publication was substituted.

## Publication disposition

This branch is an implementation checkpoint for the blocked task, not an approved
integration candidate. No PR or default-branch integration is submitted. The
operator-directed close must record `--outcome fail --work-outcome blocked`,
`crisp-nexus-12`, both unmet named deliverables and the outstanding review. The
final guarded-push OID and close receipt belong to the task's durable history.
