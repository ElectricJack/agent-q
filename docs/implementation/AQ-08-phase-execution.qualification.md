# Fresh phase execution owner qualification

Source `e934dd4`, targeted corruption-fixture compatibility `53916d0`, and final insert-freshness fence `3af7741` implement state 32. [The machine report](AQ-08-phase-execution-evidence/report.json) inventories the exact source files, pinned unchanged admission 0.1.9 archive, retained logs and qualification limits. The peer's two actual-worker assembly files are excluded from this owner source checkpoint and require separate qualification.

The internal owning function admits an authenticated, exact phase Prepare against the actual accepted receipt and original sealed choice. It loads immutable inputs, releases SQL locks for bounded fresh Graph/project/Git/private reads, then rechecks current project, registry, principal, qualified InventoryV2 compatibility epoch, positive availability and first-unissued lineage budget under authority locks. It never reruns selection or restamps the original proposal/source observations. Protocol/host capabilities come from the actual admitted and currently qualified runtime; the installed harness separately proves exact CLI/adapter/environment/model configuration and policy capabilities.

One transaction retains the verified installation claim, prepared journal, hot Grant and immutable phase request/receipt/provenance. Existing accepted/material/fact/input retrieval clocks constrain issuance. The execution lease and hard lineage/wall limit are independently anchored to actual issuance; exact replay preserves the original identity and deadlines. Legacy initial-Grant readers receive an explicit phase variant and refuse it. No legacy preparation or GrantedLaunch body is synthesized.

The shared Accept/Grant quota includes actual phase row bytes and material claims; current32 missing storage refuses startup and shared admission. Logical retained limits are not a physical PostgreSQL disk bound. Node/attempt locks precede project/catalog, registry/inventory/principal and the shared quota suffix. External owner I/O runs outside these locks. Private phase reads have their own bounded thread/queue owner, and source reads use the initial/bulk owner rather than continuation reserves.

The final fence rechecks observation freshness, every mandatory fact deadline, execution expiry and backwards clock movement after the awaited INSERT and before commit. Its deterministic fixture blocks the actual AFTER INSERT, crosses the fact window while preparation remains live, and proves the entire transaction rolls back. A separate case issues a real phase receipt, then proves legacy renewal refuses its origin without changing held state.

Qualification is deliberately separated:

| Run | Result |
| --- | --- |
| Initial actual owner matrix | 12 passed, 30.16s |
| Final owner matrix including the two new fences | 13 passed; one initial-Accept expiry before the issuer, 32.20s |
| That same original-proposal-expiry case, individually | Passed, 3.50s; unchanged clocks and TTLs |
| Accepted/origin, Started, closure, schema31/32 regressions | 39 passed |
| Legacy initial-Grant/renewal/presence aggregate | 39 passed; two failures; seven declared ignores |
| Those two legacy cases, individually | Both passed, 1.32s and 1.61s; unchanged clocks and TTLs |
| Current-owner and full-context pure checks | Six passed |
| Strict server all-target/all-feature Clippy | Passed, 44.97s |

The failed aggregates remain retained. The legacy quota test timed out observing its expected wait; the presence case returned 409 before its changed-backing assertion. An earlier corruption-fixture run also retained two DROP failures caused by the new FK; the targeted fix removes that FK only in those disposable corruption setups, keeping state32 storage present so missing state30 refusal remains isolated. Temporary nonsecret stage diagnostics identified a runtime-versus-harness capability mismatch and were removed before qualification. Distinct original pre-export and publication source observations remain preserved; their artifact identities are not incorrectly equated.

No single fully passing final aggregate is claimed. Tests use the disposable PostgreSQL from `scripts/test-db`, synthetic qualified Graph/Git/runtime peers unless a separate report explicitly proves otherwise, and no provider calls. Original two-second timely acceptance remains load sensitive; this work does not widen it or claim saturated-start capacity.

There is no production phase Prepare route or default owner activation. Metadata32 startup refusal does not fence an already-open older daemon writer. Production activation still requires a qualified writer/deployment boundary and phase-aware paired Started/Stopped, original-authority renewal/presence/closure consumers, plus worker durable grant/host integration. Source, task and effect settlement remain held under their owners; a phase Grant is not task success, verified stop, or billing evidence.
