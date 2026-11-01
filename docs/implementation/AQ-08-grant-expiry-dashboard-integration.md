# Native grant, expiry and dashboard integration

Assembled source `a7f7665` passed 45 tests across initial grants, native offers,
preparation, expiry, NeverGranted and dashboard reads, with no failures or ignores.
Strict all-feature lint for the affected library/test targets and workspace
formatting passed. [Retained evidence](evidence/aq08-grant-expiry-dashboard-a7f7665/artifact-sha256.json)
binds source hashes, commands/results, browser output and binary provenance.

The existing isolated preview was rebuilt at `960ffc7` (only documentation and
evidence changes after `a7f7665`) and passed all four actual-daemon browser checks
in 2.7 seconds. Its schema27 fixture remains in place. Overview, projects,
sessions and workers remain available; unfinished terminals/actions/workflow
views remain explicitly unavailable. Native scheduling and optional playbook
owners are off in this preview. No agent was launched.

The selected daemon tests cover actual publication and Accept→Prepare→Grant,
original grant replay, concurrent exact requests, credential rotation and
rollback, plus the independent ungranted expiry owner. An issued grant remains
held even without Started and after its deadline; expiry cannot erase it. These
are actual SQL/router checks with synthetic Graph/Git/runtime facts. They do not
qualify physical worker installation or execution, a live model, renewal,
granted-before-Started recovery, settlement or throughput. Counts overlap the
component evidence and are not additional unique product tests.
