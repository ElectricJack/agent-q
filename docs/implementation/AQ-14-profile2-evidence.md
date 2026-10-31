# Frozen catalog2/host1.2 component evidence

Source74ba611dfb0c90e59b0775a6ff2f5f882dd10f60 consumes exact producerd3bd349,
archivebd1246e8e972ee1627e94598945f398812c89044106b463151a696511412ca39.
The adjacent JSON hashes source files, preserved public definitions and all12
retained logs. Old477 vendor files plus248 new files pass drift checks. Catalog1,
frozen review0.1.0 and host library implementation bytes remain unchanged; host
package1.2 changes its exact dependency definition and documentation.

At the clean frozen source, `cargo test -p aq-server --features test-support
--test playbook_bridge --test playbook_contract --test playbook_review_contract
--test profiled_results --locked --offline -- --test-threads=2` passed29 cases.
Two current-retention pure tests passed. Server+host strict all-target/all-feature
Clippy and formatting passed. Database tests used only scripts/test-db's explicit
loopback55432 PostgreSQL. Tables026 were explicitly installed without changing
state24; registered026 fixtures instead assert normal table/column presence and
never repair a migration gap.

The genuine SQL/protected-engine path now passes lost callback→same Accepted
handle→Waiting before answer, owner resolution, another question's cancellation,
grant revocation, completion event commit/replay without AQ ACK, restart and no
question recreation. Source freshness expiry is refused after an observed budget
row wait while exact historical admission replay still works. Input/native/Git
fixtures remain synthetic; this is not selected native CLI/real Git qualification.

Host1.2 was independently compiled/tested from an exact full Git source tar,
SHA256 bc9fead6699cb0d233f16d8592e53bbc7857235c640733ef1d3a4efd1a1cad27
(69,089,280 bytes), extracted outside any checkout. No source/manifest rewriting
or sibling runtime checkout was used. With isolated target and incremental off,
locked/offline host tests passed8 normal +1 explicitly invoked PostgreSQL case;
strict all-target lint and format passed. Its recorded dependency tree excludes
AQ server and the synthetic CLI, despite unused workspace sources being present
in the archive. This is source-profile qualification, not registry publication.

See AQ-14-profile2-adoption.md for exact owner boundaries and remaining work.
AQ14 remains in progress: executed checks, native/source end-to-end and root's
ordered025/026 integration are separate. Default construction remains unmounted;
no deployment/provider/Discord or PB04 performance completion is claimed.
