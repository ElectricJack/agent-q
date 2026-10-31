# Bounded public definition owner checkpoint

Source `562e130` owns exact nonsecret configured definition bodies through the
actual inline artifact owner. Missing bodies refuse. Private validated fields
bind exact content hashes; a compact configuration authority projection preserves
old empty-field digests without repeatedly hashing large bodies. Complete config
serde retains bodies for exact roundtrips. Draft027 serializes cross-project
identity/version immutability under the existing artifact quota lock.

21 focused tests pass:3 definition owner/config tests,4 native registration,
2 existing configuration and12 project/principal/installer tests. One existing
real-peer test is declared ignored. Strict server all-target/all-feature Clippy
and formatting pass. Concurrent conflicting bodies cannot claim the same
identity/version; actual receipts and missing-body refusal are exercised.

Draft027 is explicitly installed only in component tests and remains unregistered
until026. This is not ordered migration qualification. Synthetic public evidence
bodies confer no real adapter/model/tokenizer qualification. Native producer
orchestration is still incomplete. Per-artifact receipts can remain charged after
a later retention failure; no runnable authority follows from those receipts.

[Exact commands, frozen git-show SHA256 inventory and log hashes](AQ-08-definitions.qualification.json)
are retained with [the logs](evidence/aq08-definitions-562e130).
