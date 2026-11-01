# Worker output to dashboard — first integration

The worker now captures bounded literal output from its actual tmux host, waits for the durable Started acknowledgment before terminal registration, and uploads through the existing outbound relay. An authorized dashboard Watch reads that output through the daemon. No inbound worker connection or new service is required.

The independently built worker, actual Git/tmux host, production daemon routes, and production dashboard bundle passed together in **9.48 seconds**. Chromium displayed three real frames in **613 ms**, with one Watch mutation and no page errors. The program was a periodic synthetic harness, not a model/provider. See [process/browser result](process-browser.log) and [browser assertions](browser.json).

Source: daemon `24b23c5`, worker runtime `2f92563`, dashboard `fd081fd`. Worker binary SHA256 `efbeae46ef1f438034501f4045366b4632c337afdfdc3a56ebf11bb76d9fd4ec`; test binary `d817f9f4d0179d4b735ea23d452e29145434440f7cad7077ab40b039606b1367`.

Validation includes five actual phase/HTTP tail cases, strict server lint, worker bounded packet/ACK and actual host lifecycle tests, and **254 dashboard tests**. The actual composition proves that registration, current authorization, Watch, upload, physical stop and browser display agree.

The UI explicitly requests a Watch of at most 15 seconds. Reads return at most 64 frames / 64 KiB of raw bytes; the browser retains at most 64 KiB. Output is incomplete literal text, with explicit gaps and disconnection. Current permission failure or Watch expiry clears browser output. No terminal bytes are placed in browser storage.

Output is memory-only. Restarting the daemon or allowing all Watches to expire can discard it; a restored database does not restore output. The refreshed preview therefore shows the actual stopped session and offers Watch, but honestly reports output not retained after restart. The positive live-output screenshot is an integration artifact, not seeded preview data.

This does not complete AQ-08, WRK-08, WRK-14 or UI-03. Real harness launching, phase renewal, worker no-Started closure publication, settlement/release, complete terminal rendering and interactive input remain separate work.
