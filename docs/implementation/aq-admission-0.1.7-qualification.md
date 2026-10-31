# Admission0.1.7 standalone control contract

Source `0c376e0` passes53 tests after extracting the exact package into an independent
workspace, plus strict all-target Clippy and formatting. The retained local archive
SHA256 is `46283490c98c5b082aa8dbfd4ccde230db3d97828fcbd71ff5b0526837bae545`.
[Exact qualification](aq-admission-0.1.7-qualification.json) retains source, dependency
and log hashes; repository tests/evidence are in `evidence/aq08-control-017-0c376e0`.

This adds bounded NativeOffer polling, preparation/initial grant delivery wrappers,
running renewal and read-only reconciliation. Fresh per-transmission nonce/clock
metadata is separate from immutable operation receipts. Same-revision expiry may
not change. New renewal revisions can extend only a still-live local grant and
remain below its original hard deadline. Reconciliation carries no launch permit.

For reproduction, extract the package and the exact Session0.1.0/Lifecycle0.1.0
archives into one directory. Copy the retained consumer-Cargo.toml/Cargo.lock to
that directory as Cargo.toml/Cargo.lock; run `cargo test -p aq-admission --locked
--offline` and strict all-target Clippy. Extracted manifests remain unchanged.
The original0.1.6 archive and prior contract bytes remain frozen.

This is a locally retained dependency artifact. Native025 producer routes, worker
durable driver, actual installed materials and real harness qualification remain
separate implementation work; no registry publication or live provider is claimed.
