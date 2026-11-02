# Source-retention exact-credential qualification

Production delta `fca9337107feb7096fdc193d18c4bcfb602eeca9`, following plan
`984e172`. The production fix and its existing source-retention tests are
independent of the still-unqualified physical phase-fixture commits.

HTTP callers retain the existing opaque AuthenticatedNode verifier. Before each
transaction's SourceRetention authority, the owner rechecks that exact current
credential under the current node lock, then follows the unchanged node→attempt→
transfer order. Post-filesystem validation rejection/dispatch and post-import
success/failure decisions receive the same fence. Internal trusted UUID readers
and administrative owners remain unchanged. No wire/schema/dependency/framework
change is part of this production delta.

Current replacement credentials for the same node can replay/resume the original
immutable transfer and import operation. Old credentials cannot create an intent
after an observed node-lock wait or confirm a held import after rotation.
Uncertain physical imports remain protected; denial does not claim no effect,
discard retained transport or mint a replacement operation.

Qualified source-retention default group:10 passed,0failed,1explicit real-Git
fixture ignored,6.00s. This includes both newly controlled rotation vectors and
all existing replay, partial upload, invalid manifest/quota, late/released attempt,
unknown import, staging cleanup, symlink and nonblocking control cases. The held
invalid import reply/revocation case now expects authentication denial401 instead
of503, because failed physical replies cannot bypass the current principal fence.
The synthetic socket peer isolates auth/recovery behavior; these cases do not
establish physical Git retention or a real host/provider run.

Affected all-feature strict Clippy for lib + source_retention + the separately
compiled phase-source fixture passed36.16s. The final two-target no-run check
passed3.81s after three private Store.pool accesses in the new fixture were
corrected to its existing owned env.pool. No production visibility was widened.
Playbooks independently accepted the frozen production auth delta by source
review; it performed no builds/SQL/process runs.

[Raw default test log](evidence/source-retention-auth-fca/tests.log) and
[strict log](evidence/source-retention-auth-fca/strict.log) are retained here.
The shared SERVER target was explicitly returned to Decisions after qualification.
No canonical database/config/service, provider, harness or external human message
was touched. The two physical phase SourceRetention cases remain a separate gate:
one first Frozen functional pass exposed fixture process cleanup failure, so that
pair is **not qualified** by this credential evidence.
