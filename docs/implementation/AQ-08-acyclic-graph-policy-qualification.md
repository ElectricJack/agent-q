# AQ08 graph-policy correction qualification

Frozen source `4b7cc26` passed674 full-workspace tests with six declared ignores,
strict workspace lint and formatting in a detached clean worktree. Independently
extracted profiles0.1.1 and selection0.1.2 archives passed22 and23 tests,
respectively, plus strict lint and formatting. Every source/test/example and
original manifest archive payload was compared byte-for-byte with frozen Git
source. Original v1 catalog/installed-selection/server fixture bytes are unchanged.

[Exact source, archive and log hashes](AQ-08-acyclic-graph-policy-qualification.json)
and the [dependency DAG and scope rules](AQ-08-acyclic-graph-policy.md) state the
qualified boundary. Reproduce the extracted proof with
`python3 scripts/test-profile-selection-packages.py`; it verifies five retained
archive digests and uses only that extracted public dependency closure with
locked offline Cargo. It does not depend on a live sibling checkout.

This qualifies an acyclic contract boundary, not a provider/model/tokenizer or
native grant producer. Production startup must refuse missing qualified token
counting/model-capacity evidence. Unknown numeric token counts are not zero.
Source/material/artifact ownership and actual worker launch retain their own
runtime and component qualification gates. The real-provider/peer and explicitly
authorized external-message tests remain declared ignores; no external message
or paid provider call was made by this qualification.
