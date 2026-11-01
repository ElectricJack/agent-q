# Actual accepted-phase worker input integration

The independent worker process at `cae23c7` passes against daemon `c78093f` on
ordered state30 in **7.23seconds**. It accepts and persists the new phase lease,
waits beyond the original proposal expiry, replays its exact Accept, then downloads
original artifacts, installs actual private material, hydrates an actual Git
worktree and persists the immutable phase Prepare request. The actual payload
contains36 inline plus2 source receipts and one material receipt.

The format7 journal reaches revision26; preparation completes3.705seconds after
the original local cap while still inside the independent preparation lease.
The exact accepted receipt and first delivery remain unchanged. Central state is
still Preparing/held: no execution request is sent, no legacy acceptance/Grant or
Started is fabricated, and no central installation receipt is invented.

[Exact source, binary, clock and log evidence](AQ-08-phase-input-worker-integration.json)
retains the earlier7.49second pass before a runtime-schema assertion lint fix and
the final7.23second pass. The explicit worker smoke/HTTP union passed2+11 tests;
strict all-target/all-feature worker lint and affected daemon lint passed. The
acceptance-only process regression also passes on format7 in4.45seconds.
A wrong feature-name build preflight and the constant-assertion lint failure are
retained; neither changed runtime clocks or fixture limits.

The [worker component qualification](../../../aq-worker/docs/implementation/WRK-08-phase-input-evidence.md)
separately proves126-inline coverage with actual Git. That synthetic coordinator
result is not added to this daemon fixture count. Runtime/harness/provider/Graph
qualification remains synthetic here. This proves preparation interoperability,
not execution Grant, host launch, full settlement or fleet performance.
