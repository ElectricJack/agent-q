# Accepted phase origin boundary qualification

Frozen source `f1ece57` includes production prerequisite `ac56455` and the two negative boundary regressions. [Machine report and hashes](AQ-08-phase-origin-evidence/report.json) retain exact source and log inventories.

Actual accepted no-launch history has a distinct typed origin. Legacy initial-Grant consumers refuse that origin, and Started refuses it before its legacy hot-Grant/journal path. The history loader validates the actual proposal, accepted request/receipt and original material lease row/index associations; it does not construct a legacy preparation plan or grant. Historical reads grant no live authority.

The before-fix PostgreSQL fixture reproduced Started incorrectly accepting deliberately shaped hot Grant/prepared state beneath a real accepted receipt. Renewal already refused. The same two cases pass after the fix, leaving all held state and receipt counts unchanged. Those shaped rows are explicitly negative test inputs, not a valid phase Grant or supported recovery path.

Strict all-target/all-feature server Clippy passes. Actual disposable PostgreSQL regressions pass for ten closure cases, fourteen accepted-owner cases, six legacy Started cases and the two new origin cases. The initial-Grant aggregate retained 35 passes, two safe-expiry failures and five declared ignores. Both failed cases pass when run individually without clock or TTL changes; the aggregate failures remain evidence of the legacy short-window load limitation. This report does not claim a single fully passing aggregate run. Commands and outcomes remain in the retained logs; build settings were `CARGO_TARGET_DIR=/tmp/aq-phase30-qualification-target`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`, disposable `DATABASE_URL` from `scripts/test-db`, and two test threads except the two individual reruns.

State remains 31. Migration 032 is unregistered; phase execution issuance and its fresh fact owners are separate unfinished work. No phase Started, Renew, closure, live provider or deployment support follows from these tests.
