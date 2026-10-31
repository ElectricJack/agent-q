# First AQ host contract profile

This is the fixed first `message.send`/`aq.operation.updated` profile, generated
from the AQ-owned Rust domain registry with the exact source/package pins in
`registry.json`. This is a new namespace and does not add commands or routes to
the frozen agent-access contract. The engine/compiler model's finite Schema DSL
is the schema: no arbitrary operation names, endpoint/SQL proxy or wildcard
adapter. Synthetic request/result fixtures are public contract evidence, not a
native harness or external transport qualification.

Regenerate with `cargo run -p aq-server --example playbook_contract --locked
--offline > protocol/playbook-host/v1/registry.json`; check via the dedicated
`playbook_contract` test. The registry source is Rust, not this generated JSON.
Only real AQ11 mailbox admission is implemented. Other planned command names
remain unavailable. `queued` means a durable mailbox entry; node receipt,
harness delivery, human reading, workflow outcome and task completion remain
separate. Outbox acceptance freezes event destinations; it is not run completion.

Optional admin builder injection requires a preconfigured protected namespace,
actual acquired engine owner and immutable live owner grant. Default daemon
construction has no playbook routes. The optional API only admits retained
requests, inspects/reconciles them and drives/cancels the configured policy; it
cannot install/activate arbitrary policy or choose sender authority. Automatic
driver/startup configuration and normal migration023 registration are separately
gated. No provider, Discord or production deployment is implied.
