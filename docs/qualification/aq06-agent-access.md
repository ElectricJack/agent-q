# AQ-06 qualification (2026-10-30)

Production implementation: `crates/aq-server/src/agent_access.rs`, its `contract`
and `graph` modules, migration `0003_agent_access.sql`; operator interface and
limits are in `docs/operations/agent-access.md`.

Verified in an isolated PostgreSQL database per test with TestDb, using the
qualified graph image/source retained by API-01:

- aq-graph source `225329cd6acf5a775fd2a0b016dd1c45fb16c79b`.
- Image `sha256:807beba98dcd20b0ae335f09cae8c06ecb182a352ee78de278cb47fccb2ee3be`.
- Ordinary writer credential, synthetic graph records and hidden canaries.
  No administrator fallback in the AQ adapter.

Commands and observed results:

```
cargo test -p aq-server --lib agent_access -- --nocapture
# 9 unit tests passed.
DATABASE_URL=<disposable PostgreSQL URL> cargo test -p aq-server --test agent_access -- --nocapture
# 1 PostgreSQL + fake HTTP scenario passed; multiple authority/scope assertions.
DATABASE_URL=<disposable PostgreSQL URL> python3 scripts/test-agent-graph.py
# 1 PostgreSQL + qualified-image scenario passed; all six read shapes and all
# 13 API-01 read scope vectors passed; disposable graph resources cleaned.
cargo clippy -p aq-server --all-targets --features recovery-test-support -- -D warnings
# Passed. Owned Rust sources also passed rustfmt --check.
```

The database/fake-HTTP scenario verifies verifier-only credential storage,
compare-and-swap issuance replay, explicit rotation, wrong credential classes,
wrong node/boot/generation, cancellation, held-SQL-lock expiry on reads and
queued issuance, expiry/revocation while graph responses are held, and current
authority revalidation before returning data. Database updates complete while
the graph response is blocked, demonstrating that graph I/O holds no central
node/attempt locks. Actual root routers verify maintenance blocks issuance and
rotation while permitting existing scoped reads and owner revocation.

Paging checks cover hidden-only pages, filter-before-pagination, exact query
binding, opaque continuation, credential rotation invalidating cursors, bounded
eight-page scans with explicit partial coverage, response-byte refusal and
node-budget partial coverage. Production projections remove private metadata,
actors and provenance from revisions/history/search/context, filter hidden
relationship endpoints and pinned revisions, and reject workspace mismatch.

The nine unit tests also compare canonical profile/context digests against
published API-01 vectors, enforce strict unknown fields/duplicate JSON keys and
signed-64-bit revision tokens, check conditional schema constraints, and verify
that graph error bodies are replaced by generic errors.

This is a bounded local integration qualification. No live model/provider,
production deployment, 500-agent scaling result, WRK-05 token-file launcher,
creation-derived read grant or AQ-07 mutation delivery is claimed. Large
relationship scans may explicitly return partial coverage without a cursor.
Service-token privilege class is configured by the operator because the
upstream token is opaque; runtime validates file protection and never falls
back to an administrator identity. A lost issuance response requires explicit
next-version rotation; plaintext issuance recovery is intentionally absent.

Full ContextResponse and node issuance envelopes are bounded, including profile,
binding and credential metadata. Provision reserves worst-case credential version
widths and issuance validates before committing a verifier. Regressions verify
that oversized provision leaves no binding and oversized issuance leaves version
zero and no verifier. Upstream responses must explicitly identify graph API 1;
missing or changed response-version headers are refused before projection.
