# aq-observation-api

Small portable read-only contracts for session events and usage. The CLI can use these values without depending on the daemon or accounting reducer. Existing lifecycle API0.1.0 and CON03 artifacts remain unchanged.

Event pages bind an exact session, a stable node-sequence snapshot, ordered source reports and historical accounting markers. `contribution_recorded` describes an immutable past decision; a later correction may supersede it. Native cost and completeness fields inside source records remain claims. Usage views distinguish known category subtotals, unknown totals, partial calls and unresolved observations; token categories can overlap and must not be blindly added.

Decoders reject duplicate/unknown fields, changed scope, invalid continuation, inconsistent aggregates and oversized input before use. Event pages allow16 items and256 KiB; usage views allow16 KiB. These types convey no control, source-registration or accounting authority. The server must enforce current user/session ACLs for every page.

`artifacts/examples.json` contains a sanitized synthetic CON03 source-shaped report with a missing cumulative baseline. It is not selected-harness or provider qualification. Run `cargo test -p aq-observation-api`; regenerate schemas with `cargo run -p aq-observation-api --example export -- crates/aq-observation-api/artifacts`.
