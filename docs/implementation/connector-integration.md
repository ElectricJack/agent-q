# Standalone connector and minimal execution acceptance

**Status:** AQ-01, INT-01, GATE-01 and GATE-02 complete for the local immutable candidate.

**Implementation revision:** `09634b61d0016e8816a667c3091ac0a2efbdaef5`.

The definitive image-build inventory and the final updater fixture inventory were each compared byte-for-byte with `git show 09634b6:<path>`; every entry matched. The parent independently verified the image inventory mapping. The small subsequent smoke-harness correction is retained by exact file hash and source in the evidence archive; it changes address polling and heartbeat-container selection, not runtime image inputs.

## Selected inputs and ownership

| Input | Selected identity |
| --- | --- |
| Standalone worker source | `aq-worker` commit `cf87edffa3384a08f5bb0319b4f92ffc2a26b145` |
| Vendored worker archive | SHA-256 `6f54954b2ab38e9625fd5680fa70c63f512bf7e5ef74ec19d815e562fe176e15` |
| Worker dependency lock | SHA-256 `be5c1196b5e029a7913cca2b801654df4e53cb59418f48834f8472e6adcfecad` |
| Worker package version | `0.1.0` |
| Public execution contract | `aq-worker-protocol =0.1.1`, Cargo archive SHA-256 `b421f42963c9b20b58a7a82dce96b0bbb9518e5fbb1a57069f26108f77e983f9` |
| Preserved foundation contract | `aq-protocol =0.1.0`, Cargo archive SHA-256 `8141cf1b30c2f098d2ff244e891cd71636426e0fdbb430f8351cc9f2f9343a90` |
| Daemon image | `sha256:789e00e7781b2aef5a54f0a577f69522125bfa804099cb2dbd786dd0b97447e5` |
| Standalone worker image | `sha256:73729d91e80ea27de449e1b55ddf1aab460cd206e43d85bdffd7254e97c76b65` |

[`vendor/worker-candidate.json`](../../vendor/worker-candidate.json) selects the worker source archive; [`vendor/worker-contracts.json`](../../vendor/worker-contracts.json) records the independently packaged public contracts. This is an immutable local source distribution candidate, not a public release. There is no mandatory sibling source checkout or private registry.

The daemon workspace now owns server, release and updater crates. Its two wire dependencies consume the retained public packages. The old production connector and duplicate foundation protocol source have been removed. Independent connector tests belong to `aq-worker`; daemon route tests and assembled-image compatibility checks remain in `agent-q`. The [BASE-01 report](BASE-01.md) retains the pre-extraction foundation and compatibility fixtures.

`Dockerfile.worker` builds the exact standalone archive and packages `aq-worker`, `aq-fixture-agent` and the selected source manifest. The worker target cache is keyed by the verified archive digest. Both Dockerfiles hold the target cache lock through build and binary copy; the daemon clears all local workspace/contract package artifacts before building, avoiding stale archived mtimes. `scripts/dev-worker` uses the same archive and a target directory scoped by archive digest. Its explicit `AQ_WORKER_BIN` override is printed in diagnostics and rejected by official candidate checks.

## Reproduction

```sh
python3 scripts/check-worker-candidate.py --official
scripts/smoke
scripts/execution-smoke
scripts/update-lifecycle
```

The common smoke assembly freezes its source inputs, creates a UUID Compose project with disposable secrets/state and loopback listeners, and removes its resources on completion. `AQ_SMOKE_SERVER_IMAGE` plus `AQ_SMOKE_WORKER_IMAGE` select a prebuilt pair; `AQ_SMOKE_OFFICIAL=1` requires digest references. No existing development ports, containers, volumes, credentials or data are reused. The updater suite uses a unique registry on an explicitly allocated high loopback port and proves both the WSL client and Docker daemon can reach it before building releases.

`AQ_SMOKE_EVIDENCE_DIR` and `AQ_UPDATE_EVIDENCE_DIR` select retained evidence destinations. CI retains foundation, execution and updater evidence; release verification tests its selected image digests before publishing the release manifest image. No workflow was published or deployment changed during this qualification.

## Accepted behavior

| Check | Result |
| --- | --- |
| Enrollment, heartbeat and probe using the extracted image | Pass |
| Unsupported foundation protocol 99 | HTTP 426 with foundation range 1–1 |
| Unsupported execution protocol 99 | HTTP 426; foundation node remains online |
| Daemon and PostgreSQL restart | Same saved node ID and credential; later heartbeat |
| Cancel an unaccepted offer before worker acquisition, then offer another task | Cancelled offer is declined; next offer executes |
| Real worker runtime and deterministic child | One durable launch claim; one session/attempt; stopped writer proof |
| Lost acceptance/report acknowledgements | Driver deliberately discards each first response; identical retries settle the original grant/report |
| Result replay and daemon restart | Original accepted disposition and persisted attempt returned |
| Credential revocation | Probe HTTP 401 `credential_revoked`; heartbeat connector exits 3 |
| Extracted updater lifecycle | All seven cases pass with current state format 2 and future refusal format 3 |
| Candidate/development/cleanup validation | All 11 checks pass, including floating references, override/archive/contract drift rejection and injected cleanup failures |
| Independent development command | Pinned archive builds without sibling source and reports `aq-worker 0.1.0`; explicit executable override works |

The accepted synthetic attempt is `f62941a7-19cc-402a-9e53-7215b0ac9afe`, with launch `65ded6a0-8cdc-4744-8be0-f2497b38d7f0` and report `02a92306-7d6a-4ea9-8741-98f41231baf3`. The worker journal contains one retired attempt with that launch identity and accepted report. A repeated report returns the same disposition after the worker has retired its local synthetic launch directory. This is synthetic execution completion, not graph task completion.

The updater run proves first install, upgrade, unhealthy-image rollback, kill-9 interruption/recovery, incompatible-state refusal before pull without replacing the running server, downgrade refusal through the server-writable spool, and preservation of both marker rows. Worker source remains pinned across synthetic daemon versions. These versions are generated from the committed implementation by changing workspace versions; they are not historical released binaries or a historical state-format 1→2 migration.

The [daemon validation](execution-validation.json) separately retains the 235-test extracted workspace run, including 19 execution tests, formatting and strict Clippy. The [worker report](../../../aq-worker/docs/implementation/worker-candidate.md) is component evidence; the current exact-image acceptance is recorded here. Review findings and their dispositions are in [execution-review.md](execution-review.md).

## Evidence and limits

[`connector-integration.evidence.json`](connector-integration.evidence.json) records immutable input hashes, exact commands/timings, image and executable identities, result bodies, source inventories and cleanup confirmation. [`connector-integration.logs.tar.gz`](connector-integration.logs.tar.gz) retains definitive stdout, public updater manifests/signatures/journal/status, validation check sources/results, the qualified harness and excluded development attempts. No worker credential config, enrollment response, signing private key or vault is retained. Both accepted fixtures removed their exclusively created resources; no `aq-product-*` containers remain.

Earlier attempts are explicitly excluded: prototype HTTP status expectations, transient Docker Desktop listener allocation, heartbeat/one-off selection, a Docker-daemon-unreachable dynamic registry port, and the pre-final cache policy. These were fixture or packaging issues and are retained rather than counted as acceptance.

This closes local connector compatibility and minimal distributed fake-agent execution only. It does not certify real model harnesses, Git/source handoff, central artifact retention, graph completion, full GATE-03 crash/reconciliation coverage, hosted authentication/ingress, public signing or deployment, off-host restore, capacity, or host power loss. Foundation wire version 1 and release-manifest format 1 retain their existing meanings; execution protocol 2 and worker journal format 1 are separately identified by the worker candidate metadata. Composite product upgrade inventory remains later work.
