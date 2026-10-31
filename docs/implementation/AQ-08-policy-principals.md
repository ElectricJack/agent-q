# Project-policy decision principals

Scheduled selection needs a daemon-owned authority identity. It must not borrow
the user who last edited a project, invent a user UUID, or require an operator
session to remain logged in. Migration017 adds a separate principal
beside the existing immutable decision artifact. Decision JSON and its digest
remain unchanged.

`resolve_trusted` uses a current human Owner principal. The new internal
`resolve_for_project_policy` derives its principal from the daemon's configured
project policy; callers supply neither an actor nor a policy identity. The exact
source-policy artifact binds the project, global configuration revision/digest,
enabled state, priority, source selection and retention handle. Configuration
alone is insufficient: the installed `admission_source_policies` row must match
its revision, digest and complete record. Installation and disabled tombstones
belong to the profiled runtime startup owner, not the resolver.

SQL enforces mutually exclusive variants. Human decisions require an actor and
no service-policy fields. Project-policy decisions require a null actor and an
exact project, policy artifact, revision and digest. Human operation keys retain
their existing `(actor, operation)` namespace. Policy operations use
`(project, immutable policy digest, operation)`. Same IDs in those distinct
namespaces cannot borrow each other's receipt. Changed request bodies conflict.
The existing sealed-decision trigger makes the principal immutable too.

Both initial resolution and final publication check current principal authority.
The final offer fence rechecks it while the project/node/catalog/selection locks
remain held. It rechecks time after the final principal lock wait; a decision
that expired while waiting cannot authorize an offer. A disabled, replaced,
missing or differently configured project policy cannot be revived by replaying
its old decision. Human role revocation affects the human namespace; it does not
silently remove an independently installed project policy.

The service principal only authorizes selection under the existing project
binding. It grants no graph access, execution, credential delivery, source
mutation or terminal control. Those still require their actual owning adapters
and final admission checks. No administrative HTTP route accepts a service
principal or an impersonation switch.

The adapter retains the existing global decision bounds (1024 entries and64 MiB
of decision bodies). Additional principal metadata is bounded per row to a
2 KiB artifact plus fixed scalar fields; this is not a physical disk bound.
The integrated resolver preserves migration015's finite never-granted-offer
budget and all-offer history revision. Native admission still owns any
reservation-specific eligibility checks.

Qualification uses the actual project-binding and selection services with fake
authoritative external peers. It covers current-policy installation, immutable
service provenance with no human actor, independent replay namespaces, changed
payloads, current human-role revocation, policy disablement and an observed SQL
policy-lock wait crossing the decision deadline. The original isolated
qualification explicitly installed017; integrated tests now use the normal
ordered migration chain.

## Source-policy installation follow-up

`profiled_execution::policies::install` is a daemon-owner service, with no public
HTTP route or actor argument. The runtime owner must invoke it before dispatch
and when configured projects acquire their real project binding. It computes the full configuration
digest once, installs bound policies, and reports unbound projects as ineligible.
It creates no project, workspace or human identity.

A reserved `meta` record (`profiled_policy_configuration_v1`) retains the global
revision and digest even when there are no projects or policy rows. Otherwise an
older daemon could restore a removed configuration after an empty installation.
The installer uses one advisory owner lock, then project predicates before source
policy updates. Policy records, omission tombstones and the configuration owner
commit atomically. Same revision/content is idempotent; lower revisions and
same-revision changed content refuse. Retained policy history is capped at1024
project rows, with individual records bounded to64 KiB. Unknown legacy policy
shapes require an explicit owner migration rather than being guessed into the
new authority scheme.

Removing the local configuration section makes this daemon's installer a no-op;
it does not claim to revoke another installed configuration. To retire all
policies, install a higher explicit revision with an empty projects list. Removed
projects retain disabled records and their prior source identity. This service
must be invoked by the runtime owner before dispatch; the final resolver fence
still verifies current configured and installed policy on every admission.

## Ordered integration

Migration017 is now registered after015/016 and advances the runtime state format
to17. Resolver fixtures use the normal migration chain. Selection retains the
NeverGranted started-count exclusion, all-offer history revision and finite retry
bound introduced by015, together with the fresh principal fence and post-lock
expiry check. The policy installer remains an internal service awaiting native
startup/dispatch and project-binding wiring.
