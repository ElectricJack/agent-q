# aq-review-protocol 0.1.0

Bounded, canonical review subjects and human/check receipt **claims**. The only
AQ protocol dependency is exact `aq-session-protocol =0.1.0`. This library does
not authenticate a person, confirm source retention, execute a check, or evaluate
result eligibility. Those facts require the corresponding live AQ owners.

A subject pins the exact task CAS/content revision, launch identity, retained
outcome/policy, recorded result and source-owner artifacts. An acknowledgement
from the result-record owner is not a human approval. Human answers, expiry and
cancellation have distinct closed variants; incomplete checks cannot pass.

JSON objects reject unknown fields. Canonical SHA256 uses recursively sorted
object keys and compact UTF8 JSON, bounded to 64 KiB. IDs, positive revisions,
source object formats, comments and collection cardinalities are checked before
canonical hashing. A digest binds bytes, never authority. Current role, policy,
source ownership, executor evidence and supersession must be checked by the
backend even when the DTO validates.

`artifacts/fixtures.json` is synthetic contract evidence. It includes an Unknown
check, and synthetic source and human claims; it is not an actual source review
or executed check. The upstream outcome/result-record fixtures are preserved to
prove their recording semantics remain distinct. `examples/export.rs` generates
schemas/fixtures via `cargo run -p aq-review-protocol --example export-aq-review-protocol`;
Rust drift tests compare the checked-in artifacts.

Run `cargo test -p aq-review-protocol`, strict all-target Clippy, and
`python3 crates/aq-review-protocol/scripts/check_vectors.py`. The Python checker
independently reproduces six canonical digest vectors, including the retained
upstream result-record bodies. Standalone package qualification is recorded
separately; no registry publication or production integration is implied.
