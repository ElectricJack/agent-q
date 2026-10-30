# Operations

How releases are built and promoted, how the host updater (`aq-updater`) is
installed and run, and what to do when an update needs an operator.

The provisioning of the real host (Bluehost VPS, Cloudflare Tunnel and
Access) and the live demonstrations of self-update belong to the
hosted-access plan; see [Not covered here](#not-covered-here).

## The pieces

- **Images** on GHCR, always referenced by digest:
  `ghcr.io/electricjack/agent-q-server`, `ghcr.io/electricjack/agent-q-worker`.
- **Release image** `ghcr.io/electricjack/agent-q-release:<version>`: a
  `FROM scratch` image holding only `manifest.json` (version, source revision,
  the two image digests, protocol range, state formats, upgrade range) and
  `manifest.sigstore.json`, its cosign bundle.
- **Channel** `ghcr.io/electricjack/agent-q-release:stable`: the release image
  that `promote.yml` last pointed it at.
- **`aq-updater`**, a host service outside the containers and the only thing
  that runs `docker`. It installs a requested release into the fixed Compose
  project, rolls back on failure, and recovers after a crash or reboot.
- **The server** never gets the Docker socket. It writes update requests into
  the spool and reads the updater's `status.json` and `maintenance` marker
  from it.

Trust: the updater accepts a release only if `cosign verify-blob` accepts its
manifest bundle for this identity and issuer:

```
--certificate-identity-regexp '^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$'
--certificate-oidc-issuer https://token.actions.githubusercontent.com
```

That is, the manifest was signed by `release.yml` running on a `vX.Y.Z` tag of
this repository.

**Pinned cosign: v3.1.3.** `release.yml` and `promote.yml` install exactly this
version (`sigstore/cosign-installer` with `cosign-release: v3.1.3`), and hosts
use the same version. Change all three together.

## Host layout

| Path | Owner, mode | What |
| --- | --- | --- |
| `/usr/local/bin/aq-updater` | `root:root 0755` | the updater (`aq-updater-x86_64-linux` from the GitHub Release) |
| `/usr/local/bin/cosign` | `root:root 0755` | cosign v3.1.3 |
| `/etc/agent-q/updater.toml` | `root:aq-updater 0640` | updater config |
| `/etc/agent-q/agent-q.env` | `root:aq-updater 0640` | operator env file: `AQ_VAULT_DIR`, `AQ_SECRETS_DIR` |
| `/etc/agent-q/vault/` | `root:root 0755`, files `0644` | the vault, mounted read-only into the server (UID 10001 must read it) |
| `/etc/agent-q/secrets/` | `root:aq-updater 0750`, files `0644` | `postgres_password`, `tunnel_token` (bind-mounted as-is, so readable by the container users) |
| `/opt/agent-q/compose.yaml` | `root:root 0644` | the production Compose file, from the release tag |
| `/var/lib/agent-q/updater/` | `aq-updater:aq-updater 0750` | updater state: `journal.json`, `history.jsonl`, `release.env`, the lock |
| `/var/lib/agent-q/updater/spool/` | `aq-updater 0755` | the spool shared with the server |
| `/var/lib/agent-q/updater/spool/requests/` | `aq-updater:10001 2770` | server writes request files here (read-write mount) |
| `/var/lib/agent-q/updater/spool/status/` | `aq-updater 0755` | `status.json`, `maintenance` (read-only mount) |
| `/var/lib/agent-q/backups/` | `aq-updater:aq-updater 0700` | pre-update `pg_dump` backups, the last 5 kept |

`state_dir` may use only the characters `A-Z a-z 0-9 . _ / + @ : -`: its spool
path is written unquoted into `release.env`, and the updater refuses a config
that breaks this rule.

## Install the updater on a host

Everything below runs as root on the host. It assumes Docker Engine with the
Compose v2 plugin (`docker compose version`).

### Users and groups

The updater runs as a dedicated user, `aq-updater`, in the `docker` group.

> **Membership of the `docker` group is equivalent to root.** Anyone who can
> run `docker` can mount the host filesystem into a container. The dedicated
> user keeps the updater's files, logs and registry credentials apart from
> other accounts; it is not a sandbox. Give nobody else access to it.

The server container runs as UID/GID 10001. A host group with GID 10001 lets
the server write requests and the updater read them:

```sh
groupadd --gid 10001 agent-q            # the server container's group
useradd --system --create-home --home-dir /home/aq-updater \
  --shell /usr/sbin/nologin --groups docker,agent-q aq-updater
```

(If GID 10001 is taken on the host, use that group instead; only the number
matters.) The home directory holds the updater's Docker login
(`~/.docker/config.json`) and cosign's Sigstore trust root cache
(`~/.sigstore`).

### Directories and the spool

```sh
install -d -o root -g root -m 0755 /etc/agent-q /etc/agent-q/vault /opt/agent-q
install -d -o root -g aq-updater -m 0750 /etc/agent-q/secrets
install -d -o aq-updater -g aq-updater -m 0750 /var/lib/agent-q/updater
install -d -o aq-updater -g aq-updater -m 0700 /var/lib/agent-q/backups
install -d -o aq-updater -g aq-updater -m 0755 /var/lib/agent-q/updater/spool \
  /var/lib/agent-q/updater/spool/status
# Once, as root: the server (GID 10001) writes here, the updater reads.
install -d -o aq-updater -g 10001 -m 2770 /var/lib/agent-q/updater/spool/requests
```

The updater re-applies the modes on every start (`requests/` 2770, `status/`
0755) but, not being root, leaves the group of `requests/` as set here. The
setgid bit gives every request file group 10001. The server writes request
files with mode 0640, so the updater reads them through its membership of
group 10001.

### cosign

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/sigstore/cosign/releases/download/v3.1.3/cosign-linux-amd64
curl -fsSLO https://github.com/sigstore/cosign/releases/download/v3.1.3/cosign_checksums.txt
grep ' cosign-linux-amd64$' cosign_checksums.txt | sha256sum -c
install -m 0755 cosign-linux-amd64 /usr/local/bin/cosign
cosign version
```

### aq-updater

The binary is attached to each GitHub Release as `aq-updater-x86_64-linux`
(static, built without the `test-verifier` feature), with a `SHA256SUMS`
file. The repository is public, so download both directly:

```sh
base=https://github.com/ElectricJack/agent-q/releases/download/v0.1.0
curl -fsSLO "$base/aq-updater-x86_64-linux" -fsSLO "$base/SHA256SUMS"
sha256sum -c --ignore-missing SHA256SUMS
install -m 0755 aq-updater-x86_64-linux /usr/local/bin/aq-updater
aq-updater --version
```

### Compose file, vault, secrets and the operator env file

- `/opt/agent-q/compose.yaml`: `compose.yaml` from the tag you install
  (`git show v0.1.0:compose.yaml`). The updater replaces images, not the
  Compose file: when a release changes `compose.yaml`, its notes say so, and
  you copy the new file before installing that release.
- `/etc/agent-q/vault/config.toml`: a production vault
  (`environment = "production"`, `auth.mode = "cloudflare_access"`) with the
  spool section, which is what turns the update API on:

  ```toml
  [updates]
  spool_dir = "/updater"
  ```

- `/etc/agent-q/secrets/postgres_password` and `tunnel_token`, mode 0644.
- `/etc/agent-q/agent-q.env`, the operator's env file. It holds only the
  operator's paths:

  ```sh
  AQ_VAULT_DIR=/etc/agent-q/vault
  AQ_SECRETS_DIR=/etc/agent-q/secrets
  ```

  Do not put `AQ_SERVER_IMAGE` or `AQ_UPDATER_SPOOL` here; see the next
  section.

### Env files and precedence

The updater runs Compose as:

```
docker compose --project-name agent-q --project-directory /opt/agent-q \
  -f /opt/agent-q/compose.yaml \
  --env-file /etc/agent-q/agent-q.env \
  --env-file /var/lib/agent-q/updater/release.env  ...
```

- The updater owns `<state_dir>/release.env`. It holds `AQ_SERVER_IMAGE` (the
  active release's digest-pinned server image) and
  `AQ_UPDATER_SPOOL=<state_dir>/spool`.
- `release.env` is the last `--env-file`, so its values override the
  operator's env file.
- On a fresh host, with no release installed yet, the updater writes a
  placeholder image (`no-release-installed.invalid/agent-q-server@sha256:000…`)
  so that Compose can parse `compose.yaml` and start postgres.
- **Variables in the process environment override every env file.** Never
  export `AQ_SERVER_IMAGE` or `AQ_UPDATER_SPOOL` for the updater: not in its
  systemd unit, not in your shell before a manual `aq-updater` command.
  `sudo -u aq-updater` resets the environment, which is one more reason to run
  the commands that way.

To run Compose by hand the same way (logs, `ps`, starting postgres), define:

```sh
aqc() {
  sudo -u aq-updater docker compose --project-name agent-q \
    --project-directory /opt/agent-q -f /opt/agent-q/compose.yaml \
    --env-file /etc/agent-q/agent-q.env \
    --env-file /var/lib/agent-q/updater/release.env "$@"
}
```

### updater.toml

```toml
state_dir = "/var/lib/agent-q/updater"
backup_dir = "/var/lib/agent-q/backups"
# Defaults, shown for reference:
# release_repo = "ghcr.io/electricjack/agent-q-release"
# channel = "stable"
# readiness_deadline_secs = 120
# poll_interval_secs = 3600
# min_free_bytes = 2147483648
# backups_keep = 5
# expected_auth_mode = "cloudflare_access"

[compose]
project_dir = "/opt/agent-q"
files = ["/opt/agent-q/compose.yaml"]
env_files = ["/etc/agent-q/agent-q.env"]
project_name = "agent-q"

[verifier]
kind = "cosign_keyless"
cosign_path = "/usr/local/bin/cosign"
identity_regexp = '^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$'
oidc_issuer = "https://token.actions.githubusercontent.com"
```

```sh
install -o root -g aq-updater -m 0640 updater.toml /etc/agent-q/updater.toml
```

### Registry login (`docker login ghcr.io`)

The packages are private. Create a GitHub token that can only read packages:
a classic personal access token with only the `read:packages` scope (or a
fine-grained token limited to reading packages, where GitHub accepts those for
GHCR). Log in **as `aq-updater`**, because Docker reads the credentials of the
user that runs it:

```sh
sudo -u aq-updater -H docker login ghcr.io -u <github-user> --password-stdin <<<'<token>'
```

The credentials land in `/home/aq-updater/.docker/config.json`. If you ever run
the updater as root instead, root's `/root/.docker/config.json` is used and
needs its own login (and files root creates in the state directory then lock
the service out; see [Manual commands](#manual-commands-over-ssh)). Rotate the
token by logging in again; nothing else stores it.

### systemd unit

`/etc/systemd/system/aq-updater.service`:

```ini
[Unit]
Description=Agent Q host updater
Documentation=https://github.com/ElectricJack/agent-q/blob/main/docs/operations.md
Wants=network-online.target docker.service
After=network-online.target docker.service

[Service]
Type=simple
User=aq-updater
Group=aq-updater
ExecStart=/usr/local/bin/aq-updater --config /etc/agent-q/updater.toml run
# SIGTERM goes to aq-updater only. It finishes the step in progress (an
# install included) and starts nothing new; the docker commands it is
# running are not signalled, so they complete.
KillMode=mixed
KillSignal=SIGTERM
# Long enough for an in-flight install: pull, backup, replace, probe, and
# a rollback with a restore. SIGKILL only after that.
TimeoutStopSec=2700
# Exit 3 = halted mid-update (the journal keeps the step); the restart
# runs recovery. Exit 2 = config or usage error, which a restart cannot fix.
Restart=on-failure
RestartSec=10
RestartPreventExitStatus=2
UMask=0027
# No Environment= for AQ_SERVER_IMAGE or AQ_UPDATER_SPOOL: the process
# environment would override release.env.

[Install]
WantedBy=multi-user.target
```

Do not enable it yet: do the first install by hand (next section).

## First install on a fresh host

1. Initialise the updater's state. This creates the journal, re-applies the
   spool modes and writes `release.env` with the placeholder image and the
   spool path. It installs nothing:

   ```sh
   sudo -u aq-updater aq-updater run --once
   cat /var/lib/agent-q/updater/release.env
   ```

2. Start postgres (the updater backs up and probes through it):

   ```sh
   aqc up -d --wait postgres
   ```

3. Install the release, which pulls and verifies it, starts the server and
   waits for it to report ready:

   ```sh
   sudo -u aq-updater aq-updater install 0.1.0
   sudo -u aq-updater aq-updater status
   ```

4. Start the rest of the stack (cloudflared waits for a healthy server) and
   enable the service:

   ```sh
   aqc up -d
   systemctl daemon-reload
   systemctl enable --now aq-updater
   journalctl -u aq-updater -f
   ```

From now on, updates are requested through the API.

## Requesting an update through the API

Only an owner may request an update; any signed-in user may read the status.
The API sits behind Cloudflare Access, so use `cloudflared access` (it opens a
browser login once and then adds the Access token):

```sh
cloudflared access login https://aq.example.com

# Status: state, active / previous / available releases, recent history.
cloudflared access curl https://aq.example.com/api/updates

# Install a specific version.
cloudflared access curl https://aq.example.com/api/updates \
  -X POST -H 'X-AQ-Request: 1' -H 'Content-Type: application/json' \
  -d '{"version": "0.2.0"}'

# Install the stable channel's current release.
cloudflared access curl https://aq.example.com/api/updates \
  -X POST -H 'X-AQ-Request: 1' -H 'Content-Type: application/json' -d '{}'
```

A request is answered with `202 {"request_id": "…"}`; the updater picks it up
within a few seconds. `X-AQ-Request: 1` is required on every mutation (CSRF
protection). Errors:

| Status | Code | Meaning |
| --- | --- | --- |
| 400 | `invalid_version` | not a plain `X.Y.Z` |
| 403 | | not an owner |
| 409 | `update_in_progress` | an update is running or a request is already queued |
| 503 | `maintenance` | the server is in maintenance during an update (writes are refused) |
| 503 | `updates_unavailable` | no `[updates]` in the vault, or the spool is not readable |

The updater never installs automatically: the hourly channel poll only fills
in `available`. The updater validates every request itself (signature,
upgrade path, state format, no downgrade) and records refusals in the history.

## Manual commands over SSH

Run the updater's commands as `aq-updater`, so the files they create stay
writable by the service and the right Docker login is used:

```sh
sudo -u aq-updater aq-updater status
```

- `aq-updater status` takes no lock and writes nothing: run it at any time,
  including during an update. It prints the state, the active, previous and
  available releases, and the last 10 history entries. With a torn or
  unreadable `journal.json` it prints the raw file and exits 1 instead of
  guessing.
- `aq-updater install <version>` and `aq-updater restore-previous` take the
  update lock, which the running service holds. **Stop the service first**,
  then start it again:

  ```sh
  systemctl stop aq-updater          # waits for an in-flight update to finish
  sudo -u aq-updater aq-updater restore-previous
  systemctl start aq-updater
  ```

  Without the stop, they exit 1: "The update service holds the lock while it
  runs".
- `aq-updater restore-previous` makes the previous release active again. If
  both releases use the same state format it swaps the images only. If they
  differ, it refuses unless you add `--restore-backup`, which restores the
  backup taken before the active release was installed. **Every change made
  since that backup is lost.**
- `aq-updater verify-manifest <manifest.json> <manifest.sigstore.json>` checks a
  manifest against the configured verifier (exit 0 valid, 1 invalid, 2 usage
  error).

Exit codes for all commands: 0 success; 1 failure (refused, rolled back,
signature rejected, lock held, corrupt journal); 2 usage or config error; 3
halted mid-update (the next start recovers).

### History detail and logs

History details (in `status`, `status.json` and `GET /api/updates`) are
deliberately short: for a failed command in a container they name the
command, its exit status and the first error line. `status.json` is readable
by every signed-in user, so full output never goes there. The full stderr is
in the journal of the service:

```sh
journalctl -u aq-updater --since '1 hour ago'
```

(For a manual `install` or `restore-previous`, it is on your terminal.)

## `failed_needs_operator`

**What it means:** an update failed, the updater tried once to bring back the
release that was running before, and that failed too. It does not loop:

- the maintenance marker stays on, so the server (if it runs at all) refuses
  writes with `503 maintenance`;
- nothing further is tried automatically, including after a restart or
  reboot (recovery sees the recorded outcome and waits);
- `aq-updater status` shows `state: failed_needs_operator`, the detail, and the
  next steps.

The state can also come from a recovery that cannot tell what happened (for
example, history written by a newer updater after an updater downgrade). The
updater never acts on a guess.

**Steps:**

1. Read the state and the detail: `sudo -u aq-updater aq-updater status`.
2. Find the cause: `journalctl -u aq-updater`, `aqc logs --tail 200 server`,
   `aqc logs --tail 200 postgres`, `aqc ps`, `df -h /var/lib/docker
   /var/lib/agent-q`. Fix what you can (disk space, registry login, a stopped
   Docker daemon).
3. Stop the service so the next command can take the lock:
   `systemctl stop aq-updater`.
4. Retry bringing back the active release. `status` names the command:
   - `sudo -u aq-updater aq-updater restore-previous` when the data was not
     changed by the failed release;
   - `sudo -u aq-updater aq-updater restore-previous --restore-backup` when it
     may have been (the failed release used another state format). This
     restores the pre-update backup named in the detail; changes made after
     it are lost.

   Success prints a `rolled_back` entry ("operator restore: … is running
   again") and clears maintenance. The retry is a single attempt; if it fails,
   the state stays `failed_needs_operator` with the new detail, and you can
   repeat steps 2 to 4.
5. Start the service again: `systemctl start aq-updater`.
6. If the retry cannot succeed (for example the backup is missing), restore by
   hand: the backups are `pg_dump --format=custom` files in
   `/var/lib/agent-q/backups/`, newest first by name. Do not delete
   `journal.json` to get out of this state; ask for help instead.

## Cutting a release

1. On a branch, bump `[workspace.package] version` in `Cargo.toml` and run
   `cargo update --workspace` so `Cargo.lock` matches (release builds use
   `--locked`).
2. If the release changes the database state format, update
   `STATE_FORMAT_VERSION` and `READABLE_STATE_FORMATS` in
   `crates/aq-server/src/lib.rs`, and the `io.agent-q.state-format` labels in
   the `Dockerfile`. If it cannot upgrade from every earlier release, narrow
   `UPGRADE_FROM` there too. The updater refuses releases that cannot read the
   installed data; a release that only reads a new format needs a tested
   migration and restore procedure first.
3. Open a PR, wait for `ci` to pass, and merge it.
4. Tag the merge commit on `main` and push the tag:

   ```sh
   git tag v0.2.0 <merge-commit>
   git push origin v0.2.0
   ```

5. `release.yml` runs on the tag (`gh run watch`). In order, it:
   - checks the tag equals the workspace version;
   - builds and pushes the server and worker images, stamped with the tagged
     commit;
   - generates `manifest.json` with the new server image's own
     `aq-server manifest`;
   - signs it keyless with `cosign sign-blob` (the workflow's OIDC identity);
   - builds the static `aq-updater` and verifies the manifest with it, using the
     production identity;
   - runs `scripts/smoke` against the pushed digests;
   - only then pushes `agent-q-release:<version>`;
   - creates the GitHub Release with `manifest.json`,
     `manifest.sigstore.json`, `aq-updater-x86_64-linux` and `SHA256SUMS`.

   The release image is pushed only after the smoke test passes, and the
   updater and `promote.yml` find releases only through it. A run that fails
   before then leaves at most server and worker images, which no host can
   install. If it fails, fix the cause and cut a new patch version. Do not
   move the tag.
6. Promote it to the stable channel:

   ```sh
   gh workflow run promote.yml -R ElectricJack/agent-q -f version=0.2.0
   gh run watch -R ElectricJack/agent-q
   ```

   `promote.yml` requires the GitHub Release to exist (that is, the release
   run finished, smoke test included). It resolves
   `agent-q-release:<version>` to its digest, verifies the manifest and bundle
   in that image with cosign and the production identity, checks the manifest
   names the version, and only then points `agent-q-release:stable` at that
   digest (`docker buildx imagetools create --prefer-index=false`).
7. Hosts show the new release as `available` after their next channel poll
   (within an hour). Request the update through the API.

## Upgrading the updater by hand

Releases do not replace `aq-updater` itself; the spec defers that. To upgrade
it:

```sh
gh release download v0.2.0 -R ElectricJack/agent-q -p aq-updater-x86_64-linux -p SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
sudo -u aq-updater aq-updater status        # expect state: idle
systemctl stop aq-updater                   # waits for an in-flight update
install -m 0755 aq-updater-x86_64-linux /usr/local/bin/aq-updater
aq-updater --version
systemctl start aq-updater
sudo -u aq-updater aq-updater status
```

Upgrade only while idle. A newer updater reads the older journal and history.
Avoid downgrading the updater: history it does not understand makes recovery
stop at `failed_needs_operator`. Also re-read the release notes for changes to
`updater.toml` or the unit.

## Installing a worker release by hand

Workers run on other machines and are not updated by `aq-updater`. Install
them from the worker image digest in a verified manifest:

1. Get the manifest and bundle from the GitHub Release and verify them:

   ```sh
   gh release download v0.2.0 -R ElectricJack/agent-q -p manifest.json -p manifest.sigstore.json
   cosign verify-blob --bundle manifest.sigstore.json \
     --certificate-identity-regexp '^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$' \
     --certificate-oidc-issuer https://token.actions.githubusercontent.com \
     manifest.json
   WORKER=$(python3 -c 'import json; print(json.load(open("manifest.json"))["images"]["worker"])')
   echo "$WORKER"     # ghcr.io/electricjack/agent-q-worker@sha256:…
   ```

2. On the worker machine, log in to GHCR with a `read:packages` token as
   above, then pull the digest:

   ```sh
   docker pull "$WORKER"
   ```

3. First time only, enroll. Create a code with
   `POST /api/enrollments` (owner), then:

   ```sh
   docker run --rm -i -v aq-worker-config:/config "$WORKER" \
     enroll --server https://<worker-endpoint> --name "$(hostname)"
   ```

   The worker's identity is kept in the `aq-worker-config` volume.

4. Run it, or replace the running one, by digest:

   ```sh
   docker rm -f aq-worker 2>/dev/null || true
   docker run -d --name aq-worker --restart unless-stopped \
     --read-only --cap-drop ALL --security-opt no-new-privileges:true \
     -v aq-worker-config:/config "$WORKER"
   ```

   `GET /api/workers` shows the node online again. The worker and the server
   must share a protocol version; the manifest's `protocol` range says which.

## Not covered here

These belong to the hosted-access plan; see it for the procedures:

- provisioning the VPS, Cloudflare Tunnel and Cloudflare Access, and the
  worker endpoint;
- signing out (logout) and the rest of the phase 1 access items;
- the live demonstrations on the real host: a successful self-update, a
  failed one, and an interrupted one with a reboot.

Off-host encrypted backups and restore drills belong to the recovery proof
plan.
