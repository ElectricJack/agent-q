# Worker lifecycle and dashboard integration

Schema 33 is registered and the worker listener now serves authenticated phase Prepare, Started, Stopped, launch-closure and history routes. Prepare remains inactive unless installed execution profiles have matching cleanup qualifications. The existing grant owner still checks current authority and all original phase requirements. Cleanup authenticates through the separate control pool; history and acknowledgments never grant new execution permission.

An independent worker now prepares real Git inputs, receives a grant through the production router, runs a synthetic harness through real tmux, proves kernel ECHILD, reports Started/Stopped, and durably retains both daemon acknowledgments at journal revision 36. The daemon records Finished/Stopped while retaining task/source/effect claims. This process passed in 8.14 seconds. No real model or provider was invoked.

The dashboard adds authorized recorded start/stop times and central execution state to the session roster and details. It validates the retained receipt pair under the existing session access checks and adds no database round trip to the existing offer lookup. Process observation remains unknown and terminal controls remain unavailable.

## Verification

| Check | Result |
| --- | --- |
| Production router, real worker/Git/tmux, durable paired ACKs | 1 passed, 8.14 s |
| Production Prepare HTTP and refusal boundaries | 4 passed, 7.28 s |
| Final full response-envelope guard | 4 passed, 7.24 s |
| Terminal HTTP/body/auth/control-pool isolation | 3 passed, 1.65 s |
| Actual lost grant reply, history-only recovery | 1 passed, 6.79 s |
| Preparation after proposal expiry | 1 passed, 10.40 s |
| Dashboard SQL/human API composition | 6 passed, 4.11 s |
| Terminal policy and header unit checks | 4 passed |
| Started owner and clock/rollback checks | 10 passed, 25.95 s |
| No-Started closure owner | 4 passed, 9.72 s |
| Schema 33 upgrade/writer/capacity validation | 9 passed, 23.76 s |
| Original phase fixture regressions | 14 passed, 33.96 s |
| Frontend focused checks | 34 passed, typecheck/build passed |
| Actual live browser, no mocks | 4 passed, 2.5 s |

The paired Stopped owner separately passed four tests, including late cleanup, exact replay, deferred-commit rollback and refusal of unqualified legacy result/release writes. Final all-target/all-feature server Clippy passed; the later response-envelope guard has its own affected-target checks. Earlier failed test setups and superseded draft qualifications remain separate; this table describes the final named scopes, not one aggregate suite. The current legacy-parent test qualifies genuine current-schema integrity; earlier frozen-source legacy migration evidence is distinct from this run.

Five warmed dashboard detail requests on a tiny fixture took 11.50–13.17 ms (median 12.50 ms). The joined receipt lookup used primary-key indexes and executed in 0.167 ms. These samples are not fleet benchmarks. Existing per-session list reads remain; no claim of fleet-scale performance follows.

## Preview and remaining work

The local preview at http://127.0.0.1:4188 uses the retained database from the successful physical worker test. Session `a3493b73-56e0-5186-aea1-af548a9ae036` has actual daemon-recorded start/stop receipts. The former schema-32 preview database remains intact, with a private backup outside the repository. No receipt rows were fabricated for the preview.

Production provider execution, phase renewal, the worker no-Started closure publisher, result settlement/resource release, and dashboard live terminals/attach/nudge remain unfinished. The current ten-second first-profile lease and other eligibility policies were not widened. AQ-08, WRK-08, WRK-14 and UI-03 remain in progress.
