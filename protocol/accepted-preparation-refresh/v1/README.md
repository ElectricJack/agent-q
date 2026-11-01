# Accepted-preparation registry refresh, profile 1

This is a distinct pure `aq-admission` 0.1.10 contract. It validates structural
correlation and bounded time arithmetic; it authenticates no owner, performs no
current reads and returns no execution permit. Server/worker/terminal adoption
and actual static-process qualification remain separate.

Use `aq_admission::preparation_refresh_v1`, including `registry_revision` and
`validate_registry_chain`. Its typed phase names parallel `preparation_v2` but
have their own negotiation, schema and digest domains. The frozen 0.1.9 API,
artifacts, errors and timing behavior remain unchanged. Neither profile decodes
the other's messages as compatible fallback.

The exact artifact ID is
`aq.admission.preparation-registry-refresh-v1-contract`, version `1`; the actual
SHA is in [contract.json](contract.json). The [schema](schema.json) has URN
`urn:aq:admission:preparation-registry-refresh:1`. Version `2` in document bodies
identifies the existing accepted-preparation transport family, whose immutable
SQL field paths remain unchanged. The new exact contract and four required
features select the new profile from Proposal onward. A version number alone
is not negotiation. The old feature enums/schema are not extended.

The accepted lease requires `registry_at_accept`. A Registry fact requires four
actual refs: `selected_registry` equals original `proposal.choice.registry`,
`accepted_registry` equals that accepted lease floor, `preflight_registry` is the
first locked Prepare owner read and `current_registry` is the final issuing read.
All use qualified owner ID `aq.intelligence-registry`, identical actual config
digest and canonical positive decimal revisions within signed64 range, ordered:

```
selected <= accepted <= preflight <= current
```

`FactBody.revision` is the actual final registry revision. The fact also retains
and checks the exact original credential profile/revision. Any config or
credential identity change, lower revision or false original/accepted association
refuses. Equal refs are valid when actual separate owner reads see unchanged
state. Aggregate observation-only advances do not invalidate the proposal solely
because their revision changed. The original selection/choice is never restamped.

Current principal, authorization, configuration, complete relevant health,
negative/cooldown/source age, credential backing, original source/material/input
retention, cancellation and runtime qualification still require authenticated
owner checks. The pure layer cannot prove those facts from hashes/DTOs or detect
an externally restored whole database without an independent watermark.

The 128 KiB document limit, 120-second maximum preparation/execution lease,
30-second maximum fact observation interval, original accepted deadline and
independent delivery/replay clock bounds remain. A new wrapper does not renew an
old source observation. All nine fact kinds and all existing context, inventory,
source, backing, budget and exact operation associations remain mandatory.

The [examples](examples.json) contain complete synthetic data vectors for all
eight document variants, including independently timed AcceptedDelivery and
ExecutionDelivery. They deliberately have `authority_admission: false`; Ready
claims in these data are not provider availability or runtime qualification.
Regenerate/check them with:

```
cargo run -p aq-admission --example export_preparation_refresh -- protocol/accepted-preparation-refresh/v1 [--check]
```

The package also retains its self-contained grouped normative vector under
`artifacts/preparation-refresh-v1.json`, plus the actual pre-refactor 0.1.9
capture. Its private typed template holds the phase/fact algorithms once, with
separate public schema names, exact negotiation/domains and registry hooks.
There is no production old-DTO conversion or duplicate timing/material engine.

See the [design and ownership boundary](../../../docs/implementation/AQ-08-registry-observation-refresh-profile.md).
Only terminal manifest/dependency pins move to 0.1.1 for workspace resolution in
this source increment; its new typed Context/profile, worker format-11 adoption
and runtime route/qualification changes belong to the Runner lane. No SQL,
canonical migration, provider call or launch is introduced here.
