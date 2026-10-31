# M3 distributed recovery qualification

**Accepted:** WRK-04 and GATE-03 for the bounded local `linux-subreaper-guardian-v1` profile. The definitive run passed **20/20 cases**, including the strengthened replay, receipt-history and helper-cleanup assertions. Review found no remaining blocker. No deployment or remote publication occurred.

## Immutable inputs

| Input | Identity |
| --- | --- |
| Daemon/product checkpoint | `23ed41dd51140cb17828075118a80dafb9ebe20e` |
| Daemon runtime implementation | `1fa3c40dc1f753ae7a3985c7a169ae2ec867086d` |
| Worker 0.2.0 source | `d680d728ea9314242809659c655d50d6de559b43` |
| Actual vendored archive SHA-256 | `34179856a8414b96539b80c207920048a42b46985ffe97e1c4c65beaa7495670` |
| Instrumented daemon image | `sha256:3ae84ba1c5c357998171ce5279b4a1eb406a4a2c79cfb1f69512bbd703135366` |
| Worker image | `sha256:3f29d8af5837367f20f79a25f2fb392ad64af8b04abb29825bb079d5af0d85be` |
| Public wire contracts | Foundation `aq-protocol 0.1.0`; execution `aq-worker-protocol 0.1.1`, unchanged |

The **actual image-build authority is `recovery_build.source_inventory`**, not the inherited `source_inventory` generated later by the common assembly helper. Of its 82 entries, 81 match `git show 23ed41d:<path>` byte-for-byte; `Dockerfile.recovery` matches the retained test-only [recipe](../../scripts/recovery-smoke-server.Dockerfile). The five retained harness sources match their recorded hashes; the actual fault-proxy copy has its own hash. The parent independently verified these mappings.

The dedicated daemon recipe enables `recovery-test-support`, which adds owner-only development clock and synthetic bulk controls. The production Dockerfile never enables this feature; ordinary-build route absence is separately tested. The shared assembly uses unique UUID projects, disposable volumes/configuration and loopback listeners. Normal recovery builds consume the pinned archive without a sibling checkout. Explicit prototype overrides cannot select an official release and are excluded from this acceptance.

## Supported process ownership

The live guardian is a Linux subreaper. It binds launch, node/boot, attempt/session/generation and spec identity before Start. It targets its actual descendants and establishes stopped writers only through kernel `waitpid` **ECHILD in that same trusted guardian**, with adopted children reaped. Missing PIDs, an empty scan, process-group disappearance or initial-child exit are not stopped proof. Started crash cases exercised a double-forked descendant that changed session/process group and wrote a counter.

Before durable Start intent, ordered journal semantics can prove nonstart. After that boundary, a surviving guardian supports stop-and-preserve recovery. Losing the guardian retains `stop_uncertain`, protected resources and zero available slots; a conflicting successor is refused. The test operator subsequently destroys that disposable PID namespace, without manufacturing a central stopped receipt. Live reattachment is unsupported.

This cooperative ancestry profile requires no delegated cgroup or extra container capabilities. It is not hostile-code confinement and does not cover writers launched through external services, tampering with private runtime/control data or a changed PID namespace without a stopped receipt. See the matching [daemon scope](recovery-authority.md) and [worker profile](../../../aq-worker/docs/implementation/worker-recovery.md).

## Definitive cases

| Case | Result |
| --- | --- |
| `lost-acceptance` | Passed: same guardian kernel ECHILD proof |
| `late-grant` | Passed |
| `crash-grant-durable` | Passed: durable never-Start proof |
| `crash-launch-intent-durable` | Passed: durable never-Start proof |
| `crash-guardian-spawned` | Passed: same guardian kernel ECHILD proof |
| `crash-guardian-identity-durable` | Passed: same guardian kernel ECHILD proof |
| `crash-start-intent-durable` | Passed: same guardian kernel ECHILD proof |
| `crash-start-sent-before-receipt` | Passed: same guardian kernel ECHILD proof |
| `crash-start-receipt-durable` | Passed: same guardian kernel ECHILD proof |
| `crash-report-durable` | Passed: same guardian kernel ECHILD proof |
| `worker-restart` | Passed: same guardian kernel ECHILD proof |
| `guardian-loss` | Passed: uncertainty and protected capacity retained |
| `partition-expiry` | Passed |
| `cancel-first` | Passed: same guardian kernel ECHILD proof |
| `result-first` | Passed: same guardian kernel ECHILD proof |
| `old-result-after-replacement` | Passed: same guardian kernel ECHILD proof |
| `daemon-restart` | Passed: same guardian kernel ECHILD proof |
| `control-under-bulk` | Passed: same guardian kernel ECHILD proof |
| `old-binary-journal2-refusal` | Passed: same guardian kernel ECHILD proof |
| `renew-expires-while-db-locked` | Passed: same guardian kernel ECHILD proof |

The acceptance-loss proxy commits upstream, pauses before dropping the TCP response, and exposes a durable barrier. The harness compares absolute grant expiry before and after retry, immutable grant fields/sequence against the worker journal, identical request hashes, and nonincreasing remaining TTL. It deliberately permits TTL aging rather than requiring identical response bytes.

Cancellation is tested on both sides of result admission. Old reports retain their original receipt after a successor starts and cannot change its outcome. Daemon restart first preserves grant/directive state, then a second restart after report commit preserves the exact report, disposition and one report/history row plus one receipt row. Held-lock renewal waits for an observed PostgreSQL lock waiter, advances past expiry, and requires refusal without extending the persisted grant.

Actual network disconnection and independently advanced worker/guardian clocks prove expiry stops the descendant writers and retains a stale result. These injected clocks make timing repeatable; this is not physical host-suspend testing. The daemon fixture offset is restored before traffic resumes after restart, and is not claimed as production clock persistence.

The historical qualified worker 0.1.0 is built from its retained `cf87edf` archive and rejects a copied actual guardian journal 2 via strict schema/version refusal before daemon contact. The copied journal remains unchanged. No private guardian token or credential file is copied into that test volume.

## Reserved control under synthetic bulk activity

Two held synthetic bulk requests occupy both bulk permits and two real general-pool connections. A further bulk request is explicitly refused as overload. Real worker renewal and polling continue through reserved control admission; cancellation stops and reaps the writers while bulk remains blocked.

| Local measurement | Result |
| --- | --- |
| Control inspection requests | 40 |
| Control inspection p95 / maximum | 1.74 / 1.99 ms |
| Cancellation recognition, conservative sampled upper bound | 508.3 ms |
| Cancellation through stopped report | 782.1 ms |
| Actual worker poll/renew responses observed under load | 7 |

Budgets were p95 <=1 second, maximum <=2 seconds, recognition <=1.1 seconds and stopped report <=2 seconds. No cross-host monotonic timestamps are compared. This shared WSL2/Docker Desktop result qualifies the declared two-bulk synthetic admission profile only. The independent daemon regression exhausts its configured general database pool while control remains available; the image fixture does not claim whole-pool exhaustion, real graph throughput or the later 500-agent capacity envelope.

## Reproduce and inspect

```sh
scripts/recovery-smoke
scripts/recovery-smoke --list
```

Use `--case NAME` for a bounded reproduction and `AQ_RECOVERY_EVIDENCE_DIR` for an explicit output location. The [machine record](recovery-qualification.json) retains every case, exact command, source/image identity, selected public journal/guardian evidence, measured latency and cleanup result. The [compressed bundle](recovery-qualification.logs.tar.gz) retains final diagnostics, exact harness sources, proxy-network validation and explicitly superseded prototype/earlier runs. [Bundle metadata](recovery-qualification.logs.json) records its digest. Only the definitive run closes acceptance.

The [daemon component record](recovery-validation.json) retains 242 workspace tests, 30 API-contract tests, strict Clippy and ordinary-build fixture-route absence. The [worker report](../../../aq-worker/docs/implementation/worker-recovery.md) retains 66 tests and strict Clippy on the selected worker source. CI wiring is present; hosted CI itself was not run during this work.

All owned containers, volumes and networks were confirmed removed with no cleanup errors. Every transient Docker helper is project-labeled, so failure cleanup can find it after a client timeout. The bundle was scanned for actual node credential values and private credential/token/key/config files; none are retained. Node enrollment responses, `worker.toml`, guardian `control.token`, signing keys and vaults are excluded.

Physical suspend/power loss, hostile-process security, whole-container live reattachment, real agent harnesses, external effects, graph/Git delivery, source retention and large-fleet capacity remain outside this qualification. Those limits do not turn uncertainty into stopped writers or free capacity.
