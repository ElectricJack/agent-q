# E5 VPS and Cloudflare Provisioning Implementation Plan

> **For agentic workers:** Execute this plan natively in the assigned AQ worktree. Owner actions go through `aq message send --to user:dashboard`.

**Goal:** Write `docs/operations/vps-install.md` and run it with the owner, bringing up the pinned production stack on a Bluehost VPS behind Cloudflare Tunnel and Access. The run must prove origin isolation, reboot recovery and off-host backups.

**Task:** smart-falcon-16.7. **Spec:** `docs/specs/0001-infrastructure-foundation.md` ("VPS provisioning and installation", "Human login and machine identity", "Configuration and persistent state", "Verification and completion").

## Decisions

- **Size (owner, 2026-11-03).** Bluehost NVMe 8: 4 vCPU, 8 GB, 200 GB NVMe. It runs the stack, the supervisor host and one cloud sandbox, with `aq-worker-cloud` limited to 1 concurrent attempt. 8 vCPU / 16 GB is the upgrade path.
- **Budget for 4 vCPU / 8 GiB.**
  - The sandbox slice gets `CPUQuota=200%` and `MemoryMax=4G`.
  - The long-running Compose services share 2.0 vCPU and 2.5 GiB.
  - The host keeps at least 1.5 GiB for the kernel, dockerd, containerd, Podman and page cache.
  - `VPS` and the reserve in `scripts/compose-workers-smoke.py` move to this size.
- **Hostnames.**
  - Human: `<human-host>`. Access app with an email OTP policy (owner allowlist) → tunnel ingress `http://server:8080`.
  - Machine: `<machine-host>`. Access app with a Service Auth policy (service tokens only) → `http://server:8081`.
  - The vault `[auth] audience` is the human app's AUD. The worker listener validates the AQ node credential itself, and Access is its outer gate.
- **Tunnel.** A remotely managed tunnel; its ingress rules are kept in the Cloudflare dashboard and recorded in the runbook. The connector token sits in `/etc/agent-q/secrets/tunnel_token`. No host port is opened.
- **Signing identity.** `^https://github\.com/ElectricJack/aq-server/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$`. The workflows themselves are fixed by smart-falcon-16.7.1.
- **Backups.** A restic repository off the VPS, holding pg_dumps of `aq_server` and `aq_graph`, plus `/etc/agent-q` (vault, env, updater.toml, secrets). Retention is 7 daily, 4 weekly and 6 monthly. The owner chooses the destination. The restic password and the destination keys live only in the owner's password manager and in root-only files on the VPS.
- **Disk.** A daily prune timer:
  - removes stopped sandbox containers and dangling images;
  - removes dangling engine images;
  - over 70% use of `/`, also removes unused sandbox images and volumes, Cargo caches included.
  - It never removes engine images that are tagged or in use, so the rollback image survives.

## Files

- Create `docs/operations/vps-install.md`, the runbook. It links to `docs/operations.md` for the updater install.
- Create `deploy/aq-prune.sh`, `deploy/systemd/aq-prune.{service,timer}`.
- Create `deploy/aq-backup.sh`, `deploy/systemd/aq-backup.{service,timer}`.
- Create `scripts/vps-checks.py` for the origin-isolation, route-isolation and Access checks, with JSON evidence.
- Create `scripts/test-vps-ops.py` to test the three scripts with fake binaries and a local HTTP server.
- Modify `compose.yaml`, `compose.workers.yaml`, `deploy/systemd/aq-sandbox-user.slice.conf`, `scripts/compose-workers-smoke.py` and `scripts/test-compose-prod.py` for the budget.
- Create `docs/implementation/E5.md`, the evidence.

## Tasks

1. **Tests first.** Write `scripts/test-vps-ops.py`, with these tests:
   - `test_prune_light_below_threshold`
   - `test_prune_deep_above_threshold`
   - `test_prune_never_prunes_tagged_engine_images`
   - `test_backup_dumps_both_databases_and_config`
   - `test_backup_fails_when_dump_fails`
   - `test_backup_applies_retention`
   - `test_checks_origin_ports_closed`
   - `test_checks_machine_host_refuses_human_routes`
   - `test_checks_flag_open_origin`

   Run `aq test scripts/test-vps-ops.py` and confirm they fail.
2. **Implement.** Write the scripts until the tests pass.
3. **Resize the budget.** Change the budget test to 4 vCPU / 8 GiB and see it fail, then change the limits. Run `aq test scripts/test-compose-prod.py`.
4. **Runbook.** Write `docs/operations/vps-install.md`. Each step gives its check, the failure, and the fix.
5. **Owner actions.** Ask the owner in one message, record a task comment, and wait. The asks:
   - buy the VPS;
   - set up DNS and Cloudflare;
   - choose the backup destination;
   - cut the release after 16.7.1 lands.
6. **Run it on the VPS.** Install the stack, then run `scripts/vps-checks.py` and the reboot check. Prove a backup, then write `docs/implementation/E5.md`.
