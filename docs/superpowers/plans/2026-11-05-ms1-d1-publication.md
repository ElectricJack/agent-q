# MS1 D1 publication: B builds on A

Date: 2026-11-05. Branch `ms1/d1` (aq-server, from `ms1/retry`). Context:
[MS1.md](../../implementation/MS1.md) finding 3: the project's source is pinned
to the base commit and D1 publication is not on the execution path, so B started
without A's `hello()`.

## What already exists (reused unchanged)

- **D1a** `delivery::DeliveryService`: `candidate.create` from server-owned
  retained-source receipts, `publish` with a frozen intent per operation ID, a
  dispatch marker committed before any socket bytes, immutable receipts, and a
  destination hold that only a conclusive reply releases (never a re-push).
- **D1b** `PublicationPolicy` on the revisioned project binding
  (`ProjectBinding.publication`, target validated against the aq-git binding's
  `allowed_targets`) and `DeliveryService::deliver`, which maps receipts to
  typed outcomes. Pull-request mode refuses explicitly (aq-git 0.2.0 cannot
  create a head branch; no GitHub adapter or credentials yet, D0/D2).
- **D3** default completion policy with `delivery = required`: the gate holds
  on `delivery_evidence_missing` until a server-owned
  `profiled_delivery_evidence` row exists ("seam for D1a/D1b delivery
  owners"), completes on `published`, reworks on `unpublished` and holds on
  `indeterminate`. Nothing writes that row today.
- **Named-ref source selection** (`SourceSelection::NamedRef`): resolved at
  offer time through a fresh `repo.observation`; the export key freezes the
  resolved commit ("a moved ref is a different export, never a followed one").

## Decision: the minimal real path

1. **Order: publish before completion, not after.** The project uses D3 with
   `delivery = required`. When D3 has accepted the result (stopped, Success,
   retained source, required checks) and holds only on
   `delivery_evidence_missing`, the new D1 delivery owner publishes the
   retained source, writes the delivery evidence, and D3 re-evaluates and
   completes. Completing first and publishing afterwards would release B before
   A's commit is on the integration branch, so B could still start from the old
   source. This is D1b's "delivery entry point that D3's default completion
   policy calls".
2. **Publication owner** (`profiled_execution/settlement/publication.rs`), per
   recorded report, only when the project binding names a publication policy:
   - Sources: the report's server-confirmed retained receipt (never a worker
     value). Candidate base and `expected_old` are the same OID: the project's
     current source head for the policy target (step 3), or, before the first
     publication, the commit the attempt was admitted on.
   - Operation IDs are derived from the report ID (candidate and publish), and
     a retry replays the inputs frozen in the existing D1a intent, so a moved
     head can never change a dispatched payload.
   - Outcome mapping: `applied` / `already_present` → `published`;
     `target_moved`, `rejected`, a conflicted candidate → `unpublished` (D3
     rework); `operation_indeterminate` or a lost reply → `indeterminate` (D3
     holds; reconcile is D1b's explicit operation, never a re-push). Transport
     errors, a blocked destination, `recovery_blocked`, other aq-git failures
     and pull-request mode write no evidence (the result stays held, nothing is
     reworked for a server-side condition).
   - Runs in a spawned task after the result is recorded, so a worker that
     drops its HTTP request cannot cancel a publication mid-dispatch.
3. **Source advance** (migration 0097, `project_source_heads`): one row per
   project, repository, binding generation and target: the published commit,
   its retention handle (the candidate's aq-git handle) and the publication and
   candidate operation IDs. Written only by the publication owner in the same
   transaction as the `published` evidence, and only as a compare-and-swap from
   the `expected_old` it published over.
4. **Ref-following source** (opt-in by configuration): a project whose source
   policy is `named_ref` on the publication target resolves the ref to its
   recorded source head (the publication receipt is the evidence) and hydrates
   from the candidate's retention handle. Without a recorded head the existing
   fresh `repo.observation` path is unchanged. The source-export owner accepts
   the recorded head's handle only for exactly that commit.

## Deliberately not done here (owners)

- GitHub remote: aq-git 0.2.0 has no credential profiles (D0 is on aq-git main
  but not pinned, A3b) and there are no central GitHub credentials (D2). The
  live proof publishes to a local bare remote cloned from
  `ElectricJack/aq-scratch` with `allow_file_transport`, which exercises the
  same aq-git push-with-lease path.
- Pull-request mode, `wait_for_checks` and the GitHub adapter (D1b/D2).
- Automatic reconcile of an indeterminate publication, and a replay sweep for
  results held across a server restart (D1b/D3).
- Following external pushes to the ref (a central sync owner that retains the
  observed head; D2). An external push makes the next publication
  `target_moved` (rework), never a silent overwrite.

## Tests

- DB test `retained_result_publishes_then_completes_and_advances_source`: fake
  aq-git peer; published evidence, head advanced by CAS, D3 confirmed; replay
  is idempotent with no socket I/O; `target_moved` → `unpublished` → rework.
- Unit tests: the admission-observation owner accepts a named-ref observation
  backed by a publication head only when ref, commit, handle and repository
  match.
- Live on a separate stack `aq-ms1d` (ports 38080/38081/38741): A → B with a
  real Claude agent; A's commit fast-forwards `main` on the remote, the source
  head advances, B hydrates A's change and B's commit builds on it.
