# Results feedback v1

Generate/check `schema.json` with `cargo run --locked -p aq-server --example
results_feedback_contract [-- --check]`. Regenerate the G1 union with
`cargo run --locked -p aq-server --example dashboard_actions_contract`.
`python check.py` validates the fixtures, UTF-8 bounds, replay identity and
read/action compatibility. Runtime tests enforce retained artifact provenance,
current authority, version races and fixed Graph keys.

All paths require the current Owner and shared CSRF/maintenance/admission guards.
Mutations return `dashboard-actions-1` receipts and share `/api/v1/operations/{id}`.
Read envelopes remain `dashboard-read-1`. The G1 union admits the seven finite
kinds `result.follow`, `result.comment`, `comment.revise`, `thread.resolve`,
`feedback.task_create`, `feedback.link`, `result.evidence_waive`.

- Results: GET `/api/v1/results?mode=latest|needs_review|following`, optionally
  `scope=all_authorized|project`, `project_id`, `task_id`, `media=any|image|none`,
  inclusive `since_ms`/`until_ms`, `limit` (default 20, maximum 100), `cursor`.
  Membership is frozen for five minutes; every page rechecks current authority
  and retained report provenance. Cursors bind all filters, identity and size.
  Queries spanning over 1000 eligible results must narrow their filters. At most
  128 live snapshots and eight concurrent feed/comment readers per router.
- GET `/api/v1/results/{id}/revisions/{revision}`: exact retained SHA-256 report
  revision. `id` is the supersession-root Result ID (a report ID is also accepted
  for compatibility). The immutable revision contains only native submitted
  content; native reports have no supplemental media/check/addressed-feedback
  fields, so those submitted arrays are empty. Supplemental upload references
  and evidence availability belong to the independently versioned assessment.
- PUT `/api/v1/results/{id}/follow`: Follow payload; expected Result version.
- GET/POST `/api/v1/results/{id}/revisions/{revision}/comments`: reads use
  `limit` and `cursor`; POST uses Comment payload with **result_id and revision
  omitted** (the path supplies them). G1 union payloads include both pins.
  Replies name `thread_id` and the same anchor. Reply order uses a SQL sequence;
  cursors freeze the upper sequence. Each comment retains up to 16 revisions.
  Reads expose the complete correction history. POST
  `/api/v1/comments/{id}/revisions` uses Correction and the comment version.
- POST `/api/v1/threads/{id}/resolve`: Resolve and thread version; exact
  follow-up revision must share project/task. Linking and creating tasks leave
  threads open. Thread version advances for replies and resolution.
- POST `/api/v1/feedback-tasks`: FeedbackDraft, including expected thread version,
  exact retained comment revision, project revision and optional parent task.
  The native `task.create` fixed key and task UUID both use `operation_id`.
  A parent-child edge is created atomically in the same Graph mutation.
  GET `/api/v1/feedback-tasks/{operation_id}` names the deterministic candidate
  task while confirmation/linking is uncertain. `creation_disposition=confirmed`
  distinguishes a proven created task from an unconfirmed candidate.
  GET of the common operation reconciles a confirmed native receipt with its
  local link. POST `/api/v1/feedback-tasks/{operation_id}/recover` with `{}` is
  explicit retry under the original native key, including an interrupted intent
  before native admission. Read-only lookups never dispatch Graph mutations.
- POST `/api/v1/feedback-links`: FeedbackLink, thread version and currently
  authorized existing task. Origin records retain result/artifact/comment pins.
- POST `/api/v1/results/{id}/evidence-waivers`: Waiver and assessment version;
  exact requirement ID and pinned policy revision with an Owner reason.

Image anchors require finalized retained PNG evidence on that exact revision,
matching input or render digest, finite normalized bounds and positive extent.
Video anchors have a schema for forward compatibility but receive typed 422
`unsupported_media_kind`; no clip or transcoding owner is mounted.

Capture requirements live in append-only SQL `result_task_policies`, keyed by
project/task/policy revision. This is the administrative task-policy store;
policy authoring and completion-gate wiring belong to their task-policy owners.
The retained report transaction snapshots the latest policy, including every
requirement field. Historical reports receive explicit empty policy revision 0.
Policies are bounded and SQL-validated, support PNG image requirements, and
cannot be corrected in place. Each required count must match finalized, retained,
not withheld evidence with exact report, scenario, environment, viewport and
claimed source, or have an authorized exact-policy waiver. Scenario/steps and
source are human claims; this evaluation establishes evidence presence, never
capture correctness or task acceptance. Assessments remain `unreviewed` until an
acceptance owner supplies that authority; G6 performs no completion-policy write.
