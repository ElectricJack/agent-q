# M4 foundations qualification — 2026-10-30

**Accepted:** AQ-06, WRK-06 and AQ-09 for the bounded local profiles described
in [M4 foundations](m4-foundations.md). GATE-04 remains open. No remote
publication, production deployment or hosted CI execution occurred.

## Immutable implementation and assembly

| Input | Identity |
| --- | --- |
| Daemon implementation | `056b1feafef2fa1d0629bf429bec336bd7c1c1dc` |
| Final integration and worker pin | `a03c5f23f756b606a7876563e037d5d23a0e0e5e` |
| Worker 0.3.1, journal 3 | `3fb2bcb2f63f73838ddd97e44580fe721a539bdd` |
| Worker archive SHA-256 | `ed33076d62d37884cab04f1206d1fda3dd5e06181ddb1ccdebec58fda789b535` |
| Production server image | `sha256:5d7aefe6116f03dcfc0a1057bd4ba0be0f0bad800d48b4d78fd0b4893bb3142c` |
| Final worker image | `sha256:e810ebc21c3436452efbe7c9b348116983e462fd3dd758e4deec10622f155eaf` |
| Recovery-only server image | `sha256:61de3be6ec5a3305ae8513b9c455586b67fe578df76adf5f2801f164c274b773` |

All 132 tracked production-build inputs match the integration commit. All 131
tracked recovery inputs also match, mapping `Dockerfile.recovery` to the retained
test-only recipe. The frozen contexts additionally included 11 inert developer
Python bytecode/log files; their digests are recorded, and they are not Rust
compiler inputs. See [source mapping](m4-source-mapping.json). The daemon Rust
sources are unchanged from the fully tested implementation commit.

The worker protocol package remains 0.2.0 at `c3af960`; foundation wire protocol
1 and execution wire protocol 2 are unchanged. Both candidate checkers pass in
official mode. The product builds from retained archives, without sibling source
checkouts. Server and worker images execute the pinned Git 2.56.0 at the absolute
default path, and embed the same qualified aq-git binary SHA-256
`9de33fd4e79d68a9cb8c72532ee03825dca3a3dbaf979bcd8770a53ef8fe9d3a`.

## Observed results

| Check | Result and evidence |
| --- | --- |
| Daemon workspace, recovery feature | 266 passed, zero failed; one optional real-component test ignored by this ordinary command. [Commands and log hashes](m4-validation.json). |
| Worker workspace | 74 passed; formatting and strict Clippy passed. Raw logs in the evidence bundle's `worker-regression` directory. |
| API-01 | 30 tests passed; generated contract files matched. |
| Static checks | Strict workspace Clippy in default and recovery modes, Rust formatting, official component checks, YAML parsing and plan validation passed. |
| Ordinary production routes | Recovery fixture controls absent; existing authenticated route-separation regression passed. |
| Actual qualified graph | All six read shapes and 13 scope vectors passed using an ordinary writer credential, including hidden records, revocation and maintenance. [Graph record](m4-graph-qualification.json). |
| Source retention | Eight ordinary tests plus the explicitly invoked real aq-git test passed. Real import, worker-directory deletion, central candidate creation and scoped base download succeeded. |
| Actual source workspace and CLI | Exact-base workspace, guarded execution, stopped checkpoint/export, receipt binding, retirement, conditional eviction and fresh-transfer rehydration passed against the pinned aq-git binary. Final 0.3.1 changes only the unbound stopped-observation retry path; source integration was qualified on 0.3.0. |
| Production images | Enrollment, unsupported protocol refusal, daemon/database reconnect, synthetic execution with lost acknowledgements, exact report replay and revocation passed. [Production record](m4-execution-qualification.json). |
| Distributed recovery | **20/20 passed** on final worker 0.3.1, including network partition, guardian loss, all crash boundaries, cancellation orderings, stale reports, reserved control capacity, journal refusal and locked-database expiry. [Recovery record](m4-recovery-qualification.json). |
| State migration | Exact migrations 1/2 followed by current startup preserved a durable node/result receipt and prior migration timestamps; repeated startup was idempotent. Store suite: 14/14 passed. |
| Updater lifecycle | **7/7 passed**, including first install, upgrade, broken-release rollback, interrupted update, incompatible-format refusal, downgrade refusal and data preservation. [Update record](m4-update-qualification.json). |

The updater run uses a recorded development snapshot and worker 0.3.0; its
purpose is schema-4 packaging/lifecycle regression, not final-worker execution
qualification. Its copied B/C contexts and recipe hashes are retained. Its
releases all use schema 4; the separate PostgreSQL test covers migration 2→4.
This is not an end-to-end updater rollback across the schema-2/schema-4 boundary.

## Review and failure correction

Review hardened source storage accounting, transfer/artifact identity separation,
complete credential-response byte budgets, upstream graph version checking,
retention-release fencing and monotonic execution evidence. Existing report
history reconstructs conservative start evidence for older projections without
adding a history lookup to current records.

The initial recovery run exposed a real worker delivery bug: while disconnected,
the guardian stopped and reaped its writers, but an immediate observation request
failed with `Network is unreachable`. The worker exited before its result retry
loop, leaving the stopped proof, report and observation durable locally.

A deterministic actual-worker-binary regression failed before the fix and passed
after it. Worker 0.3.1 retries the same stopped observation within a bounded
30-second window, then reaches result delivery. It neither renews authority nor
relaunches. The regression checks identical requests, one acceptance, one report,
zero renewals and kernel ECHILD proof. The final real network-partition case and
full 20-case matrix passed. Failed/superseded runs are retained separately and do
not constitute acceptance evidence. Recovery failure cleanup now captures public
journals and driver diagnostics before removing disposable resources.

## Evidence and limits

The [compressed evidence bundle](m4-qualification.logs.tar.gz) retains 894 files,
including final and explicitly superseded runs, exact harness copies, raw logs,
source inventories and component checks. [Metadata](m4-qualification.logs.json)
records its SHA-256 and every member digest. AQ bearer/enrollment credentials and
private credential/key files were excluded; disposable database URLs were omitted
where present. All fixture-owned containers, networks and volumes were removed,
including the separate qualification PostgreSQL container.

The Linux cooperative subreaper profile, conservative storage admission and
bounded read/transfer profiles remain the qualification limits. There is no
large-fleet benchmark, hard filesystem quota, physical power-loss guarantee,
hostile-process confinement or live reattachment claim. Central aq-git process
composition is still explicit operator setup. WRK-05/07/08 and AQ-07/08/10 remain
necessary for the complete real-agent path; this increment does not close GATE-04.

The next independent M4 tracks are WRK-05 (agent client and context cache),
WRK-07 (outbound resumable source handoff) and AQ-07 (durable graph mutations).
