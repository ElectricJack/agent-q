# AQ14 review contract checkpoint

Exact source `9b0b4801098cf0afc031b89a01e9e97f2dfbe063`, with prerequisite
`b64db82`, defines aq-review-protocol0.1.0. It depends only on exact frozen
session0.1.0. Existing outcome/result-record bytes remain unchanged. The adjacent
JSON inventories every source-closure file and retained log with SHA256.

The exact Git source archive SHA256 is
`cc62507eb0ea9f281603b830c12e6685546a81725f311f24bbdbcbad62a7f108`.
It contains the new crate and its exact session dependency, with no root workspace
or sibling runtime checkout. This is an exact source publication profile, not a
registry .crate publication. The extracted library independently resolved cached
Rust dependencies offline; it does not assert a transitive lockfile in the source
archive.

From `/tmp/aq14-protocol-standalone`, with an isolated target and
`CARGO_INCREMENTAL=0`: `cargo test --manifest-path
crates/aq-review-protocol/Cargo.toml --offline` passed11 tests; all-target strict
Clippy, format, schema/fixture drift and six independently computed Python
canonical SHA256 vectors passed. Retained logs are under
`evidence/aq14-review-9b0b480`. No database, provider, daemon or network service was
needed for this standalone proof.

These are bounded DTO/claim checks. Synthetic fixture human/source fields are not
actual owner decisions, confirmed source or executed checks. B1 recording ACK is
not human approval; incomplete checks remain Unknown. Runtime role/source/check
verification, decision storage/API, profile2 adapter and B2 eligibility remain
separate work. An early negative test exposed serde's unit-variant extra-field
acceptance; the frozen closed empty-struct Expired variant rejects the smuggle.
