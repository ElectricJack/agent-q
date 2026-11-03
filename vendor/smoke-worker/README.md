# Smoke-only aq-worker 0.3.1 fixture

Byte-identical copy of the aq-worker 0.3.1 candidate that `vendor/` pinned
before the docker-local re-pin (smart-falcon-16.10). `scripts/product-smoke.py`
(`scripts/smoke`, `scripts/execution-smoke`, and the release gate) builds its
standalone worker from this fixture, because aq-worker 0.5.1's `run` needs a
rootless sandbox daemon and production 0.5.1 builds omit `run-execution`.

This is **not** the production worker. `Dockerfile.worker` with `vendor/`
builds aq-worker 0.5.1 for `compose.workers.yaml`. The product smoke does
not exercise 0.5.1 until a follow-up reworks it (option A).
