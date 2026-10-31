# AQ08 configured source observation

The internal source reader maps a current configured project policy and frozen
project binding to three finite public Git reads: `repo.observation` for an
explicit named ref, `inspect` for the resulting commit, and `retention.inspect`
for the configured exact handle. An immutable policy skips reference resolution.
It supplies the expected repository binding generation on every request and
checks exact request IDs, protocol, response bounds and required evidence. It
never refreshes/fetches, creates refs, mutates retention or accepts a caller URL,
path, Git command or operation name. Four observations can run concurrently;
each reply is bounded to256KiB and each exchange to five seconds.

The first supported source profile is an exact SHA1 commit directly named by an
active configured retention handle. Annotated tags, SHA256 and ancestor retention
through a descendant handle refuse. These are explicit narrower gates, not
claims that the upstream API lacks those capabilities. A failed/latest refresh,
stale/future observation, changed repository generation, released/malformed
handle, noncommit object or mismatched object refuses.

The reader retains the earlier of the original reference-observation start and
its own request start, and charges the full thirty-second freshness window from
that time. Lock/RPC waiting never refreshes it. The resulting canonical evidence
bytes include the inspected original reference/commit/retention facts, their
source admission and configured handle. A digest descriptor is returned alongside
the bytes; no durable artifact retention or runnable authority is implied.
Actual offer creation must retain these bytes, export and retain the real source
bundle, and perform the current project/policy/selection/node/source fences.
Grant preparation must re-observe the same sealed source choice rather than
reuse an expired initial reference observation. No runtime offer route is added
by this helper. Tests use a synthetic bounded Unix peer; real selected Git
qualification remains an explicit producer integration gate.
