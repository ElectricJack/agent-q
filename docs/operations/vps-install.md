# VPS install: Bluehost, Cloudflare Tunnel and Access

This runbook takes a clean Bluehost VPS to the pinned production stack behind
Cloudflare Tunnel and Access. It follows the seven steps of the foundation
spec (`docs/specs/0001-infrastructure-foundation.md`, "VPS provisioning and
installation").

- The updater, its systemd unit, `updater.toml`, cosign and the first install
  are in [`docs/operations.md`](../operations.md). This runbook says where to
  run each of those and what to change; it does not repeat them.
- Every step ends with a **Check**. If the check fails, apply the matching
  **Fix** and rerun the step.
- Every step can be rerun on a host that already has state. No step deletes a
  volume, recreates a database or rotates an identity. Steps that create a
  secret say "only if missing".
- Configuration lives in git: the Compose files, the systemd units and scripts
  under `deploy/`, and `vault.production.example/`. Credentials never do: the
  tunnel token, database passwords, service tokens, backup keys and the
  restic password are created on the host or in the owner's accounts.

The record of the real run is [`docs/implementation/E5.md`](../implementation/E5.md).

## Names used below

| Name | Meaning |
| --- | --- |
| `<vps-ip>` | the VPS public IPv4 address |
| `<domain>` | the owner's domain, on Cloudflare DNS |
| `<human-host>` | `aq.<domain>`: dashboard and admin API, for people |
| `<machine-host>` | `aq-workers.<domain>`: the worker API, for machines only |
| `<team>` | the Cloudflare Zero Trust team name (`https://<team>.cloudflareaccess.com`) |
| `<owner-email>` | the owner's email, the only identity allowed on `<human-host>` |
| `aq-sandbox` | the dedicated user that runs the rootless sandbox daemon |

Run host commands as root over SSH unless a step says otherwise. Define
`aqc` as in [operations.md, "Env files and precedence"](../operations.md#env-files-and-precedence).

## Size and plan

**Plan: Bluehost NVMe 8**, chosen by the owner on 2026-11-03: 4 vCPU, 8 GB
RAM and 200 GB NVMe. It runs the stack, the central supervisor host and one
cloud sandbox at a time. The cloud worker's maximum concurrent attempts is 1,
and that sandbox gets about 2 vCPU and 4 GB.

These limits are in `compose.yaml`, `compose.workers.yaml` and
`deploy/systemd/aq-sandbox-user.slice.conf`. `scripts/test-compose-prod.py`
and `scripts/compose-workers-smoke.py` check them.

| Consumer | vCPU | Memory |
| --- | --- | --- |
| postgres | 0.5 | 512 MiB |
| server | 0.5 | 384 MiB |
| aq-graph | 0.25 | 128 MiB |
| aq-git | 0.25 | 256 MiB |
| cloudflared | 0.125 | 128 MiB |
| aq-supervisor-host | 0.25 | 1 GiB |
| aq-worker-cloud (controller only) | 0.125 | 128 MiB |
| **Long-running services** | **2.0** | **2.5 GiB** |
| sandbox slice (`user-<uid>.slice`) | 2.0 (`CPUQuota=200%`) | 4 GiB, no swap |
| **Total** | **4.0** | **6.5 GiB** |
| Left for the kernel, dockerd, containerd, Podman and page cache | 0 | 1.5 GiB |

The one-shot init jobs run before the services start and are not counted.

**Disk.** Keep `/` under 70% of 200 GB, which leaves at least 60 GB free.
`aq-prune.timer` (step 2) enforces this: above 70% it removes every unused
sandbox image and every unreferenced sandbox volume, Cargo caches included.

**Upgrade path: 8 vCPU / 16 GB.** Use it for two concurrent Rust-building
sandboxes (E1c's sizing). On that size, go back to `CPUQuota=400%` and
`MemoryMax=8G` and E1c's service limits (`docs/implementation/E1c.md`). Set
`VPS` in `scripts/compose-workers-smoke.py` to match.

**Bluehost terms** (checked 2026-11-03; recheck before buying a prepaid term):

- The 25% CPU rule in the
  [user agreement](https://www.bluehost.com/terms/user-agreement) is for shared
  hosting. A VPS may use the resources of its plan. There is no published
  sustained-CPU cap for VPS. Before buying a term longer than a month, ask
  Bluehost sales to confirm in writing that a service running 24/7, which
  sometimes holds 2–4 vCPU for minutes, is acceptable.
- [Upgrading a plan](https://www.bluehost.com/help/article/how-to-upgrade-vps-hosting-plan)
  is done in place: Account Manager → Hosting → Upgrade. It is prorated, it
  restarts the server, and the disk and IP are kept. Plans cannot be
  downgraded. "Compute" accounts cannot upgrade individual resources.
  Step 7's reboot check is also the post-upgrade check.

## 1. Create the VPS; SSH and recovery console

1. **Owner.** Buy NVMe 8 with **Ubuntu 24.04 LTS** (or 26.04 LTS if offered),
   no cPanel, in the nearest US region.
   - Add the operator's SSH public key to `root`.
   - Record the IPv4 address.
   - In Bluehost's panel, confirm that the recovery (VNC/serial) console opens.
2. Lock SSH to keys:

   ```sh
   ssh -i ~/.ssh/aq_vps_ed25519 root@<vps-ip> true
   cat >/etc/ssh/sshd_config.d/10-aq.conf <<'EOF'
   PasswordAuthentication no
   KbdInteractiveAuthentication no
   PermitRootLogin prohibit-password
   EOF
   sshd -t && systemctl reload ssh
   ```

**Check:** from a second terminal, `ssh -i ~/.ssh/aq_vps_ed25519 root@<vps-ip> true` succeeds, and
`ssh -o PubkeyAuthentication=no root@<vps-ip>` is refused.

**Fix:** if key login fails, open the recovery console, append the key to
`/root/.ssh/authorized_keys` (mode 0600, directory 0700), and fix
`10-aq.conf`. Keep the first SSH session open until the check passes.

## 2. Firewall, image, Docker, Compose, updater, sandbox daemon

### 2a. Base image and firewall

```sh
apt-get update && apt-get -y full-upgrade
apt-get -y install ufw unattended-upgrades restic jq curl uidmap dbus-user-session
ufw default deny incoming && ufw default allow outgoing
ufw allow OpenSSH
ufw --force enable
# Bound the journal.
mkdir -p /etc/systemd/journald.conf.d
printf '[Journal]\nSystemMaxUse=1G\n' >/etc/systemd/journald.conf.d/10-aq.conf
systemctl restart systemd-journald
```

The firewall allows inbound SSH only. Cloudflare Tunnel is outbound-only, so
the origin needs no inbound HTTP(S) port. Docker can bypass ufw for published
ports, but no Compose service publishes one (`scripts/test-compose-prod.py`).
Step 5's origin probe is the real test.

**Check:** `ufw status verbose` shows `deny (incoming)` with only `22/tcp`
(OpenSSH) allowed.

**Fix:** if SSH drops after `ufw enable`, open the recovery console and run
`ufw allow OpenSSH`.

### 2b. Docker Engine and Compose

Install Docker Engine and the Compose plugin from Docker's apt repository
([docs.docker.com/engine/install/ubuntu](https://docs.docker.com/engine/install/ubuntu/)).
Then set default log rotation; the services set their own as well:

```sh
cat >/etc/docker/daemon.json <<'EOF'
{"log-driver": "json-file", "log-opts": {"max-size": "10m", "max-file": "3"}, "live-restore": true}
EOF
systemctl restart docker && systemctl enable docker containerd
```

**Check:** `docker version` and `docker compose version` answer, and
`docker info --format '{{.CgroupVersion}} {{.CgroupDriver}}'` prints `2 systemd`.

**Fix:** if `daemon.json` is invalid, Docker will not start: run
`journalctl -u docker`, fix the JSON and restart.

### 2c. Host updater

Follow [operations.md, "Install the updater on a host"](../operations.md#install-the-updater-on-a-host)
through "systemd unit". Change two things:

- In `updater.toml`, use the signing identity that releases of this
  repository carry:

  ```toml
  identity_regexp = '^https://github\.com/ElectricJack/aq-server/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$'
  ```

- Get `compose.yaml` from the release tag of the version you install
  (`https://raw.githubusercontent.com/ElectricJack/aq-server/v<version>/compose.yaml`),
  not from `main`.

Install the unit, but do not enable it yet (step 5 does).

**Check:** `aq-updater --version`, `cosign version` (v3.1.3), and
`sudo -u aq-updater docker ps` all succeed.

**Fix:** if `docker ps` fails as `aq-updater`, the user is not in the
`docker` group: run `usermod -aG docker aq-updater`.

### 2d. Sandbox user, rootless Podman and the slice

The cloud worker's sandboxes run under a dedicated rootless Podman daemon
(decision in `docs/implementation/E1c.md`). E6b enrolls the cloud worker.
This step only prepares the daemon and its cap, so that the budget and pruning
apply from the first day.

```sh
useradd --system --create-home --shell /usr/sbin/nologin aq-sandbox
grep -q '^aq-sandbox:' /etc/subuid || usermod --add-subuids 200000-265535 --add-subgids 200000-265535 aq-sandbox
SB_UID=$(id -u aq-sandbox)
loginctl enable-linger aq-sandbox
install -D -m 0644 deploy/systemd/aq-sandbox-user.slice.conf \
  /etc/systemd/system/user-$SB_UID.slice.d/50-aq-sandbox.conf
systemctl daemon-reload
```

Install **Podman 5 or later**, the version C2 qualified. Ubuntu 26.04 ships it
(`apt-get install podman`). Ubuntu 24.04 ships 4.9, so on 24.04 install a
pinned static Podman 5 build and record its version and SHA-256 in E5.md.

Then run the API socket as a user service of `aq-sandbox`:

```sh
install -d -o aq-sandbox -g aq-sandbox /home/aq-sandbox/.config/systemd/user
cat >/home/aq-sandbox/.config/systemd/user/aq-sandbox-podman.service <<'EOF'
[Unit]
Description=Rootless Podman API for AQ sandboxes
[Service]
ExecStartPre=/usr/bin/mkdir -p %t/aq-sandbox
ExecStart=/usr/bin/podman system service --time=0 unix://%t/aq-sandbox/podman.sock
Restart=on-failure
[Install]
WantedBy=default.target
EOF
chown aq-sandbox: /home/aq-sandbox/.config/systemd/user/aq-sandbox-podman.service
systemctl --user -M aq-sandbox@ daemon-reload
systemctl --user -M aq-sandbox@ enable --now aq-sandbox-podman
```

**Check:**

- `systemctl show user-$SB_UID.slice -p CPUQuotaPerSecUSec -p MemoryMax`
  prints `2s` and `4294967296`.
- `curl -s --unix-socket /run/user/$SB_UID/aq-sandbox/podman.sock http://d/v1.41/info | jq '.SecurityOptions, .CgroupDriver, .CgroupVersion'`
  shows `name=rootless`, `systemd` and `2`.

**Fix:** if `CgroupDriver` is `cgroupfs`, or Podman cannot start, the
service is not running under the lingering user manager. Check
`loginctl show-user aq-sandbox -p Linger` and the subuid line, then restart
the user service.

### 2e. Disk pruning and backup timers

```sh
install -m 0755 deploy/aq-prune.sh /usr/local/sbin/aq-prune
install -m 0755 deploy/aq-backup.sh /usr/local/sbin/aq-backup
install -m 0644 deploy/systemd/aq-prune.service deploy/systemd/aq-prune.timer \
  deploy/systemd/aq-backup.service deploy/systemd/aq-backup.timer /etc/systemd/system/
echo "AQ_SANDBOX_SOCKET=/run/user/$SB_UID/aq-sandbox/podman.sock" >/etc/agent-q/prune.env
systemctl daemon-reload && systemctl enable --now aq-prune.timer
```

`aq-prune` runs daily:

- Always: it removes stopped sandbox containers, whose writable layers hold
  the per-attempt Cargo and target caches, and dangling sandbox and engine
  images.
- Above 70% use of `/`: it also removes every unused sandbox image and every
  sandbox volume that no container references.
- Engine images are only pruned when dangling, so the active release and the
  updater's rollback image are never removed.

`aq-backup.timer` is enabled in step 4, once the backup destination exists.

**Check:** `systemctl start aq-prune && journalctl -u aq-prune -n 20` ends with
the `df` line, and `systemctl list-timers 'aq-*'` lists the prune timer.

**Fix:** if the run fails on the sandbox socket, finish 2d first.

## 3. DNS, Tunnel and the two Access applications

All of these are in the owner's Cloudflare account. Either the owner does
them by hand, or the operator does them with a scoped API token the owner
provides. Record the IDs (never the secrets) in E5.md.

1. **DNS.** Add `<domain>` to Cloudflare and switch the registrar's
   nameservers to the two that Cloudflare assigns. **Check:**
   `dig NS <domain> +short` returns the Cloudflare nameservers.
2. **Tunnel.** Zero Trust → Networks → Tunnels → Create, type Cloudflared,
   name `aq-vps`. Copy the connector token for step 4; it is a credential.
   Add two public hostnames:

   | Public hostname | Service |
   | --- | --- |
   | `<human-host>` | `http://server:8080` (admin listener) |
   | `<machine-host>` | `http://server:8081` (worker listener) |

   The tunnel creates proxied CNAMEs to `<tunnel-id>.cfargotunnel.com`.
   Neither hostname has an A record pointing at `<vps-ip>`.
3. **Login method.** Zero Trust → Settings → Authentication → add
   **One-time PIN**.
4. **Human application.** Access → Applications → Self-hosted, domain
   `<human-host>`.
   - Policy "owner": Allow, Include → Emails → `<owner-email>`. Login method:
     One-time PIN only.
   - Session duration: 24h.
   - Copy the **Application Audience (AUD) tag** for the vault.
5. **Machine application.** Access → Applications → Self-hosted, domain
   `<machine-host>`.
   - Policy "workers": action **Service Auth**, Include → Service Token →
     each worker token. No email or identity-provider rule, so a browser has no
     interactive login.
   - Turn on "401 response for Service Auth policies" if offered, so a refused
     machine gets 401 rather than a login redirect.
6. **Service tokens.** These are made by hand until E7. For each worker,
   create two tokens in Access → Service credentials → Service Tokens: one
   for the node (`aq-<worker>-node`) and one for the runner
   (`aq-<worker>-runner`), each with a 1-year duration. Add both to the
   "workers" policy. Give the client ID and secret to the worker's setup
   prompt (stdin), never as arguments. To remove a worker, revoke both tokens
   **and** the AQ credential (`POST /api/workers/{id}/revoke`).
7. **Check token.** Create one more service token, `aq-e5-check`, for
   step 5's checks. Add it to the "workers" policy, and revoke it when E5 is
   done.

**Check:** `dig <human-host> +short` resolves to Cloudflare addresses, not
`<vps-ip>`. Opening `https://<human-host>` in a private window shows the
Cloudflare Access login.

**Fix:** if a hostname shows a Cloudflare 1033 error, the tunnel has no
connector: step 5 starts it. If the `<human-host>` record is an A record to
`<vps-ip>`, delete it and let the tunnel's CNAME win.

## 4. Secrets, vault, release reference, data paths, backups

Directory owners and modes are in [operations.md, "Host layout"](../operations.md#host-layout).

1. **Secrets.** Create each file only if it is missing, so that a rerun never
   rotates a database password under live data:

   ```sh
   cd /etc/agent-q/secrets
   for f in postgres_password server_database_password graph_database_password graph_token; do
     [ -s "$f" ] || { openssl rand -hex 32 >"$f"; chmod 0644 "$f"; }
   done
   # The tunnel connector token from step 3.2; prompt, not an argument.
   [ -s tunnel_token ] || { read -rs -p 'tunnel token: ' t && printf '%s' "$t" >tunnel_token; chmod 0644 tunnel_token; }
   ```

2. **Vault.** Copy `vault.production.example/` to `/etc/agent-q/vault/` once,
   then set:
   - `[server] public_origin = "https://<human-host>"`
   - `[auth] team_domain = "https://<team>.cloudflareaccess.com"`
   - `[auth] audience = "<human AUD tag>"`
   - `[bootstrap] owner_email = "<owner-email>"`, `enabled = true` until the
     owner has signed in once, then `false`.

   Leave `admin_bind` on 8080 and `worker_bind` on 8081. The worker listener
   authenticates the AQ node credential itself; the machine Access app is
   its outer gate. So only the human app's AUD goes in the vault.
3. **Operator env file** `/etc/agent-q/agent-q.env`: `AQ_VAULT_DIR` and
   `AQ_SECRETS_DIR`, as in operations.md.
4. **Release reference.** Use the version the owner cut: a signed `v<x.y.z>` tag
   whose release workflow passed. The updater pins its digest in
   `release.env` (step 5).
5. **Backups.** The destination is the owner's bucket (Cloudflare R2
   `aq-backups` or Backblaze B2). Use an access key scoped to that bucket.

   ```sh
   umask 077
   read -rs -p 'restic password: ' p && printf '%s' "$p" >/etc/agent-q/restic-password
   cat >/etc/agent-q/backup.env <<'EOF'
   RESTIC_REPOSITORY=s3:https://<account>.r2.cloudflarestorage.com/aq-backups
   RESTIC_PASSWORD_FILE=/etc/agent-q/restic-password
   AWS_ACCESS_KEY_ID=<key id>
   AWS_SECRET_ACCESS_KEY=<secret>
   EOF
   set -a; . /etc/agent-q/backup.env; set +a
   restic snapshots >/dev/null 2>&1 || restic init
   systemctl enable --now aq-backup.timer
   ```

   **Credential-recovery decision.** The restic password and the bucket key
   are kept in the owner's password manager and in the root-only files above,
   and nowhere else. Losing both copies makes the backups unreadable; this
   is accepted. The backups contain every other credential: the vault,
   secrets and tunnel token. Restoring from a backup older than a revocation
   brings back stale worker credentials. After a restore, revoke and
   re-enroll every worker whose history cannot be trusted (the full drill is
   J5).

**Check:** `ls -l /etc/agent-q/secrets` shows five non-empty files;
`grep -cE 'example\.com|replace-with' /etc/agent-q/vault/config.toml` is `0`
(no example values left);
and `restic snapshots` answers.

**Fix:** if `restic init` fails with 403, the key is not scoped to the
bucket or lacks write access; ask the owner for a new key. Never "fix" a
database password mismatch by regenerating the secret: see step 5's Fix.

## 5. Start the stack and verify

1. Do the first install exactly as in [operations.md, "First install on a fresh host"](../operations.md#first-install-on-a-fresh-host)
   (updater `run --once`, `aqc up -d --wait postgres`, `aq-updater install <x.y.z>`,
   `aqc up -d`, `systemctl enable --now aq-updater`).
2. **Owner login.** The owner opens `https://<human-host>` on desktop and
   phone, gets an email code, and lands on the dashboard as owner. Then set
   `[bootstrap] enabled = false` and run `aqc up -d server`. An unrelated
   email gets no code.
3. **Boundary checks.** Run these from the operator machine, **not** the VPS,
   with the `aq-e5-check` token in `CF_ACCESS_CLIENT_ID` and
   `CF_ACCESS_CLIENT_SECRET`. Save the JSON in E5.md:

   ```sh
   scripts/vps-checks.py origin --host <vps-ip>      # only 22 accepts
   scripts/vps-checks.py human --url https://<human-host>
   scripts/vps-checks.py machine --url https://<machine-host>
   ```

   - `origin`: no port except 22 accepts a TCP connection on `<vps-ip>`.
   - `human`: without a login every path is refused or redirected to
     `<team>.cloudflareaccess.com`, and the service token is refused too.
   - `machine`: without the token everything is refused. With it, `/healthz`
     is 200 and `/`, `/api/me`, `/api/status`, `/api/workers`, `/api/users`
     and `/api/updates` are all 404: the worker listener has no human routes.
4. **Backup proof.** `systemctl start aq-backup && journalctl -u aq-backup -n 30`, then
   `restic snapshots` and `restic ls latest | grep -E 'aq_(server|graph)\.dump'`.

**Check:** `aqc ps` shows every service `healthy` or `running` and the init
jobs `exited (0)`. `sudo -u aq-updater aq-updater status` shows the release
active. All three `vps-checks.py` runs print `"ok": true`.

**Fix:**

- If `aq-updater install` fails verification, the release's signing
  identity does not match `identity_regexp`. Run
  `cosign verify-blob ... --certificate-identity-regexp '.*'` on the
  manifest to see the actual identity; correct `updater.toml` only if that
  identity is this repository's `release.yml` at a `v` tag.
- If cloudflared logs `Unauthorized`, `tunnel_token` is wrong: rewrite the
  file and run `aqc up -d cloudflared`.
- If the server is unhealthy with `password authentication failed`, the
  secret files no longer match the database. Restore the files from the
  backup; do not delete the volume.
- If `origin` reports an open port, find the listener with `ss -ltnp`, close
  it, and check `ufw status`.
- If `machine` returns 200 for a human route, the tunnel ingress for
  `<machine-host>` points at 8080; correct it to `http://server:8081`.

For updates, rollback and `failed_needs_operator`, see operations.md.

## 6. Enroll a worker

1. The owner creates an enrollment code: `POST /api/enrollments` from the
   dashboard session.
2. On the worker machine, run the worker's `enroll` with
   `--server https://<machine-host> --cf-service-token`. Give it the code,
   then the node token's client ID and secret, on the prompt or on stdin,
   never as arguments. The worker install itself is E6a (desktop) or E6b
   (cloud).
3. Start the worker.

**Check:** `GET /api/workers` shows the node `online`. Reusing the same code
or an expired one fails. A worker without the service token gets
"Cloudflare Access rejected the request".

**Fix:** if Access rejects the token, check that it is in the "workers"
policy and not expired. If the server rejects the credential after a
restore, revoke the node and enroll it again.

## 7. Reboot and verify recovery

```sh
reboot
# from the operator machine, after about 2 minutes:
ssh root@<vps-ip> 'docker ps --format "{{.Names}} {{.Status}}"; systemctl is-active aq-updater docker; systemctl list-timers "aq-*" --no-pager'
scripts/vps-checks.py machine --url https://<machine-host>
```

**Check:**

- Every service is back with `restart: unless-stopped` or `on-failure`, and
  `aq-updater` and Docker are active.
- The owner signs in again without a new bootstrap.
- `/api/workers` shows the same worker IDs, and the enrolled worker returns
  to `online` without re-enrolling.
- `systemctl --user -M aq-sandbox@ is-active aq-sandbox-podman` is
  `active`.

**Fix:** if the stack is not up, check `systemctl is-enabled docker`. If the
updater halted mid-step, check `journalctl -u aq-updater -b`; it recovers on
start, as described in operations.md. If cloudflared is up but the hostnames
return 1033, check its logs for the tunnel token or clock skew
(`timedatectl`).
