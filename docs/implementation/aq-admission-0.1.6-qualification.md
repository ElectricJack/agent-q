# Admission 0.1.6 standalone contract checkpoint

Source `f721c24` adds the startup delivery descriptor and bounded result-report
recording contract to 0.1.5. The exact local archive SHA256 is
`d74db09f5766a437b9d43b6d63710a56afc4db69432953ef06699ec7ab09e066`.
This is a retained local consumption artifact, not a registry publication.

All 43 package tests pass both in the repository and after extracting the archive
into an independent workspace. Strict Clippy passes for both; formatting passes.
Every preexisting 0.1.5 contract source, schema, fixture and test is byte-identical,
except for additive module declarations in lib.rs. Package metadata and README
changed. No preexisting files were removed. Exact source and log hashes are in
[the qualification record](aq-admission-0.1.6-qualification.json).

Startup delivery binds exactly one final UTF8 payload, its qualified renderer and
ordered byte witnesses for every Startup input. The reserved descriptor prefix
cannot identify alternative payload descriptors under one launch binding. Worker
consumption must pass this payload unchanged; the pure tests do not qualify a
native worker implementation, provider context limit or real provider delivery.

The result-record contract records bounded worker claims after a qualifying stop.
Its receipt says Recorded. It does not accept results, release reservations or
source, change graph state, or establish eligibility. The database report ledger
and subsequent source/evidence/effect evaluation remain separate work in
[AQ-10](AQ-10-result-settlement-plan.md).

To reproduce the standalone checks, extract the retained 0.1.6 archive and the
unchanged Session 0.1.0 / Lifecycle 0.1.0 archives into one directory. Those two
archives are retained with the [stopped-owner evidence](AQ-10-stopped-owner-qualification.md).
Copy `consumer-Cargo.toml` and `consumer-Cargo.lock` from
`evidence/aq-admission-016-f721c24` to the directory's Cargo.toml and Cargo.lock.
Run the commands in the qualification JSON. Do not edit extracted manifests.
