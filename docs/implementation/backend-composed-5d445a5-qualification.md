# Composed backend qualification, source 5d445a5

The composed state 37 backend passes 136 focused tests, strict Clippy and changed
source formatting. The default-feature executable also passes an actual
nonempty media36→37 migration on its owned disposable fixture: both original
receipts, every stored image byte and every pre-existing data/schema/sequence
invariant are preserved. This is a composed source/artifact gate, not canonical
backend promotion or completion of the parent implementation tasks.

Source is `5d445a50644b32d5318f974b9c41e4700769fc17`, tree
`d28ba3f6c0e0f3a91baae5edf29172cb7f5a21fc`. The source starts at qualified
`b23255c` (including schema 37 reservation ownership and private UDS evidence),
then imports the accepted media capacity/lock-order changes, current playbook
catalog consumer 0.1.3 and central profile/startup bridge. All imports applied
without conflicts. Shared mounted-owner flags, existing media opt-ins, the
central-authority dependency and frozen migration/archive/checker bytes remain.
Only actual live-owner-supported current-catalog inspection gains its new
capability; central profile output remains data without execution authority.

[Machine-readable qualification](evidence/backend-composed-5d445a5/qualification.json)
records original import hashes, exact commands, timing, log hashes and artifact
identity. `cargo metadata --offline --locked --no-deps` passed before compilation.
The gate reused the exclusive existing SERVER target with jobs2, debug0,
incremental0 and offline/locked dependencies. No broad workspace suite was run.

| Focused check | Passed | Wall time including any compilation |
| --- | ---: | ---: |
| Profiles, selection and playbook host packages | 71 | 17.67 s |
| Real media owner/HTTP | 24 | 78.03 s |
| Playbook activation and current catalog | 13 | 19.88 s |
| Store37 schema/data/refusal guards | 7 | 68.81 s |
| Real central reservation owner | 12 | 11.37 s |
| Historical Store36 guards | 3 | 14.08 s |
| Bounded catalog DTO/page rules | 3 | 0.23 s |
| Mounted capability/authorization | 3 | 4.39 s |

Strict Clippy passed for the three pure packages in 8.35s and server library/all
tests in 71.42s. Pure-package formatting and all 15 changed server Rust files
passed. The unchanged pre-existing module-order formatting in
`tests/profiled_results.rs` was not rewritten. Central profile/selector payloads
remain byte-identical to independently qualified `e8aa937`, including the real
21-case pre-refactor task corpus and new standalone archive closure.

## Default executable

One default-feature `cargo build -p aq-server --bin aq-server --locked --offline`
passed in 45.66s, with `AQ_SOURCE_REVISION` set to the full source commit above.
The private immutable copy (0500) is `/tmp/aq-composed-backend-5d445a5/aq-server`,
115,844,952 bytes, SHA256
`56221fc3081666f302eccf779ddd102df0fd728111645246b46e79d967f626d0`.
The build has no test-support feature. Root subsequently observed that exact
source/binary on the fixture's real readiness/HTTP listener. The cache lease was
released immediately after this single build. Later documentation commits do
not relabel the actual compiled source.

## Actual retained media36→37 preservation

After the authorized UI's second fixture upload finished, Root stopped its exact
state 36 owner. A read-only repeatable-read snapshot captured all 112 original
table bags, 2045 schema object definitions and all sequence states. A private
custom-format backup used that same exported snapshot. Actual restore into a
new disposable database matched every captured section; no service/root owner
opened the restore, and that validation database was dropped. Backups, raw
receipts and detailed manifests remain 0600 under a private 0700 directory and
are not committed.

Root then started the pinned state 37 executable on the **same** fixture database,
vault, storage UUID, managed root and original lock/marker inode. There was no
root copy, replacement marker, second concurrent owner or new upload. After
startup, read-only SQL/filesystem checks established:

- All 112 original table bags accounted for: 109 exactly unchanged. Only the state
  format, new 37 migration ledger row and notification dispatcher ID changed;
  all other metadata, original ledger rows and budget fields were exact.
- All 2045 old schema definitions and sequence states exact; 138 new schema-37 objects
  and four empty central reservation tables added.
- Both exact original canonical receipt byte strings/digests preserved; all six
  stored input/render/preview file hashes, byte counts and inodes exact.
- Root, owner.lock, objects and staging identities/modes plus marker hash exact;
  original conversation/storage/report data unchanged.
- Actual human HTTP reads returned both original receipts/digests/bindings and
  four identical preview/render PNGs, sizes and response headers. No upload or
  content/receipt rewrite occurred.

The [public preservation summary](evidence/backend-composed-5d445a5/retained-preservation.json)
contains counts/hashes only. It closes the previously explicit *nonempty retained
fixture* 36→37 gap; it does not authorize a canonical migration or establish a
production capture/harness result.

## Plan boundaries

[AQ-17](../plans/0001-v2-milestones.md#aq-17),
[API-05](../plans/0001-v2-milestones.md#api-05) and
[API-11](../plans/0001-v2-milestones.md#api-11) remain `in_progress`.
The central reservation owner and profile bridge are independently bounded
prerequisites: see [reservation ownership](AQ-17-central-reservation-owner.md)
and [central profile/startup composition](AQ-17-central-profile-startup.md).
Actual central selection, owner-retained startup/selection artifacts, HostPlan,
physical permit/material privacy, tmux startup and reply authority remain open.

The [current catalog consumer](API-05-current-catalog.md) tests use actual owners,
including authorization/stop/revision races. Its new-owner proof on the retained
private PostgreSQL UDS installation is a separate runtime gate. The
[media HTTP port](API-11-http-port.md) and
[private promotion rehearsal](API-11-media36-promotion-rehearsal.md) retain their
previous qualified boundaries; comments/feedback and broader capture remain
separate product work.

At handoff the canonical backend still runs its original state 35/8ad executable,
config and database. Its **current** promoted frontend manifest is
`624707540cc6566158c3721e4733f69934c8a1670c4ffb972afb7d469fb8e663`,
product source `f2dbe5e7fc9d2647971aab0c1350df52c54bd0a3`, with UI documentation
`8dfdef2`; it is not the earlier 5c bundle. See the separately reviewed
[frontend qualification](../../../aq-dashboard/docs/implementation/composed-frontend-6247-qualification.md).
Canonical 35→37 backup/clone/rollback/config qualification and any optional-owner
enablement remain coordinated future work. No canonical backend or main-branch
promotion was performed by this gate.
