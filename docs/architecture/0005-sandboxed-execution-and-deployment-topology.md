# Sandboxed execution, worker placement and deployment topology

**Status:** Required design. Nothing in this document is implemented yet.

**Date:** 2026-11-02

This is the canonical cross-component design for:

- running each agent attempt in its own disposable sandbox;
- building the images and snapshots those sandboxes start from ("sandbox templates");
- where worker nodes run, and how they create sandboxes locally and on remote platforms;
- how provider credentials (API keys and subscription logins) reach a sandbox;
- the first production deployment topology.

It supplements three existing documents:

- **[Execution and recovery](0002-execution-and-recovery.md)** stays authoritative for grants, stopped-writer proof, retention and task completion.
- **[Profiles, harnesses and terminals](0003-agent-profiles-harnesses-and-terminals.md)** stays authoritative for profile composition, mandatory tmux hosting and remote terminals. tmux runs *inside* the sandbox.
- **[Infrastructure foundation](../specs/0001-infrastructure-foundation.md)** stays authoritative for the Bluehost VPS, Cloudflare Tunnel/Access, enrollment and self-update.

Requirement IDs in this document use the prefixes `SBX` (sandboxed execution) and `DEP` (deployment). Short prefixes such as `R` and `P` are already used by more than one spec.

**Scope:** this document covers where and how agent work executes and how the product is deployed. The rest of the success bar is owned by the existing designs and tracked in the [implementation plan](../plans/0001-v2-milestones.md). That includes the dashboard, the supervisor conversation, task enqueueing and the v1-equivalent workflows. Section 15 lists the decisions made while reviewing this design.

## 1. Product decisions

These were decided by the product owner on 2026-11-02:

1. **One agent attempt per sandbox.** A sandbox never hosts two attempts at the same time and never runs a second attempt's processes. The single exception is a wake from hibernation: the continuation attempt may start from the hibernated sandbox's snapshot as a warm start (section 4, step 8). The sandbox is the unit of isolation, cleanup and stop proof. **This applies to worker-placed agent attempts. Central supervisor sessions are not worker attempts; they run on the VPS (section 3.1).**
2. **Agents run from sandbox templates that we build.** A template is a versioned image containing the agent-side runtime, pinned aq-git, tmux, the harness CLIs and a project toolchain. The same template runs locally and in the cloud.
3. **Worker nodes are long-lived controllers that run no agents themselves.** A worker node enrolls once and holds its node identity, capacity accounting, caches and credentials. It creates and destroys sandboxes for the attempts it accepts.
4. **Target topology:**
   - One Bluehost VPS runs the server stack and a cloud worker container.
   - The cloud worker runs `docker-local` sandboxes on the VPS itself (decided 2026-11-02 after review). boat.dev sandboxes come later through a second enrolled VPS worker node with the `boat` backend, because each worker node has exactly one backend (section 2).
   - One local worker on the owner's desktop, itself running as a container on WSL2, runs the same templates locally for extra compute.
5. **Cloud placement must support both boat.dev and Cloudflare.** boat.dev is the first remote backend. Cloudflare Containers is a second remote backend behind the same interface.
6. **Provider authentication is a mix of API keys and subscription logins** (for example, a ChatGPT login for Codex CLI or a Claude subscription for Claude Code). **Subscription logins are the primary path, and no API key is required** (product owner, 2026-11-02). API-key profiles remain supported as an option. **Both are allowed on every backend.** AQ is a personal deployment: running the owner's own agent with the owner's login on cloud compute is ordinary use, because the cloud only supplements limited desktop compute (product owner, 2026-11-02). The per-credential backend list (section 7) remains available as an operator restriction.

The success bar for this design is the product owner's definition of "finished":

- The dashboard is fully working.
- The server is hosted on the VPS.
- Cloud and local workers connect to it.
- The owner can talk to the supervisor and enqueue work the way they do in v1.
- It all works as expected.

## 2. Terms

| Term | Meaning |
| --- | --- |
| **Worker node** | An enrolled, long-lived aq-worker runtime that owns one node identity and boot incarnation (worker spec §4.1), capacity, local journal, caches and credential leases. It runs no agent processes. |
| **Placement backend** | The mechanism a worker node uses to obtain sandboxes. Initial backends: `docker-local`, `boat`, `cloudflare-containers`. Each worker node has exactly one backend. |
| **Sandbox** | One isolated execution environment created for one attempt: a container (`docker-local`, `cloudflare-containers`) or a VM (`boat`). |
| **Sandbox runner** | The agent-side runtime inside a sandbox: guardian, attempt-owned tmux host, harness adapter, aq-git client, observation and terminal transports. It is the same software that currently runs the native single-attempt path (`aq-worker run-native`), adapted to run under a delegated attempt permit. |
| **Sandbox template** | A versioned, content-addressed image definition. It is materialized as an OCI image (for container backends) and as a boat saved snapshot (for the boat backend), both derived from the same image digest. |
| **Environment** | The existing profile concept (architecture 0003 §2, worker spec §7). In this design, an environment ID resolves to exactly one sandbox template digest. |

## 3. Topology

```mermaid
flowchart LR
    subgraph CF[Cloudflare]
        Access[DNS, Tunnel, Access]
        CFC[Cloudflare Containers backend: Worker plus Durable Object, later phase]
    end
    subgraph VPS[Bluehost VPS]
        cloudflared --> Server[aq-server]
        Server --> PG[(PostgreSQL)]
        Server --> Graph[aq-graph]
        Server --> Git[aq-git, supervised]
        CW[Cloud worker node, backend: docker-local]
        VSD[VPS sandbox Docker daemon]
        V1[Sandbox container: runner plus agent]
        BW[boat worker node, backend: boat, later phase]
        Updater[Host updater]
    end
    subgraph Boat[boat.dev]
        B1[Sandbox VM: runner plus agent]
        B2[Sandbox VM: runner plus agent]
    end
    subgraph Desk[Owner desktop, WSL2]
        LW[Local worker node, backend: docker-local]
        SD[Sandbox Docker daemon]
        L1[Sandbox container: runner plus agent]
    end
    Browser[Owner browser and CLI] --> Access --> cloudflared
    CW -->|outbound HTTPS via Access| Access
    LW -->|outbound HTTPS via Access| Access
    BW -->|outbound HTTPS via Access| Access
    V1 -->|outbound HTTPS via Access| Access
    B1 -->|outbound HTTPS via Access| Access
    L1 -->|outbound HTTPS via Access| Access
    CW --> VSD --> V1
    BW -->|boat API| Boat
    LW --> SD --> L1
    CFW[Worker node, backend: cloudflare-containers, later phase] -.->|gateway API| CFC
```

### 3.1 What runs where

| Location | Runs | Does not run |
| --- | --- | --- |
| VPS, server stack (Compose) | `aq-server` (including the dashboard bundle built into the image), PostgreSQL, `aq-graph`, a supervised `aq-git serve`, `cloudflared`, and the existing host updater as a host service | Agent processes; Docker socket access for the application container (infrastructure spec, "Application container does not receive the Docker socket") |
| VPS, supervisor host container | Central supervisor CLI sessions (AQ-17): the shared tmux host and harness adapters, running under a central session identity and an expiring execution permit (architecture 0003 §4). Supervisor provider credentials come from the central vault. | Worker-placed attempts; Docker access. This is not a worker node, and supervisor credentials never reach worker nodes or sandboxes (W1). |
| VPS, cloud worker container (`aq-worker-cloud`) | aq-worker with the `docker-local` backend and its own dedicated rootless sandbox daemon. It holds its journal volume and a credential-lease cache. Agent sandboxes on the VPS have per-service CPU and memory limits, so they cannot starve the server stack. | Agent processes outside sandboxes; the host's main Docker socket; the boat API key |
| VPS, boat worker container (`aq-worker-boat`, later phase) | A second enrolled aq-worker node with the `boat` backend. It holds its own node identity, service tokens, journal volume, the boat API key and a credential-lease cache. Its sandboxes run on boat.dev, not on the VPS. | Agent processes; any Docker socket |
| VPS, agent sandboxes | One container per attempt, created by the cloud worker on its sandbox daemon | Node, backend, Git remote or supervisor credentials |
| boat.dev | One sandbox VM per attempt, forked from the template's saved snapshot | Node credentials, any other attempt |
| Desktop | aq-worker running as a container on WSL2 with the `docker-local` backend, plus a dedicated rootless sandbox daemon (section 5.1). The WSL VM is kept alive while the worker is enabled. | Agents directly on the desktop filesystem |
| Cloudflare (later) | A small Cloudflare Worker and Durable Object that creates Containers on request from an enrolled worker node | AQ server functions |

The cloud worker (and later the boat worker) runs on the same VPS as the server for cost and simplicity. If the VPS goes down, the server and the VPS worker nodes go down together, and that is accepted. Each VPS worker node still connects to the server through the public machine hostname and Cloudflare Access, exactly as a remote worker would, not over the internal Compose network. This keeps one authentication and transport path for all workers and lets a VPS worker node move to another host without changing configuration semantics.

### 3.2 Scheduling across workers

Placement stays central (worker spec §8). Additions:

- Each worker node advertises its backend kind, the maximum number of concurrent sandboxes it may create, an optional spend ceiling, the template digests it can launch, and an estimated cold-start cost per template.
- Project and pool policy orders eligible nodes by an explicit **placement preference**. The default ordering prefers `docker-local` nodes (no marginal cost), then `boat`, then `cloudflare-containers`. This default is the designer's call and is configurable per project.
- A node at its sandbox or spend ceiling refuses offers with a typed reason. Refusals are explainable (W13).

## 4. Sandbox lifecycle

Each accepted attempt goes through these steps:

1. **Reserve.** On accepting an offer, the worker node reserves one sandbox slot and the attempt's resource request (W3/W5). It records a durable journal intent *before* calling the backend, so that a crash cannot leave an unrecorded sandbox.
2. **Create.** The worker node creates the sandbox from the exact template digest in the launch manifest, records the backend's sandbox identifier in the journal, and labels it with node, boot and attempt identity. Labelling uses backend labels where they exist; boat sandboxes have no tags, so the journal mapping is authoritative there.
3. **Provision.** Sandboxes are created during *preparation*. The native protocol issues a PreparationPlan at acceptance, and the grant only after the Prepared receipts. A sandbox therefore exists, and is paid for, before the grant. The worker node delivers to the sandbox:
   - a **delegated permit**, preparation-scoped until the grant, then upgraded to execution scope (section 6);
   - the scoped AQ agent credential;
   - provider credential material (section 7);
   - the frozen launch manifest.

   Delivery goes over the backend's control channel (Docker API, boat command/file API, Cloudflare Durable Object). Nothing secret goes in the template, in command-line arguments or in environment listings visible to the agent user.
4. **Hydrate.** The sandbox runner obtains source:
   - It always fetches source from central retention through the authorized artifact export path.
   - On a cold first attempt, the **central** aq-git first fetches from the project's configured remote and retains the base. Git remote credentials stay central; sandboxes never hold them. Publication is central too (AQ-15).

   Templates may include pre-fetched objects only from repositories whose template images are stored in a private registry (section 8).

   Preparation receipts (Prepared) are submitted under the preparation-scoped permit.
5. **Run.** The runner reports Started, runs the harness in an attempt-owned tmux host under the in-sandbox guardian, streams observations and terminal output, and renews against the server (section 6).
6. **Hand off.** Before the attempt can be reported finished, retained source and result evidence must reach central retention with receipts (execution and recovery, X12). A sandbox's disk is never the only copy of work that has been reported done.
7. **Stop, preserve, then destroy.**
   - On completion, cancellation, expiry or failure, the runner stops the harness.
   - **Stopped-writer proof** is either of these backend confirmations:
     - the sandbox is destroyed;
     - the sandbox is stopped with no running processes (container stopped; boat sandbox archived).

     Either replaces per-process proof for writers that existed only inside that sandbox. Without confirmation, resources stay protected and the attempt stays uncertain (X3 rules apply unchanged).
   - **Destruction additionally requires that no unretained work remains** (W8, W14):
     - If handoff has not completed, the worker node first tries to extract the workspace and evidence through the runner or the backend file API.
     - If extraction fails, the node *stops* the sandbox and keeps its disk: container stopped with its volume kept, or boat sandbox archived (billing stops; snapshot storage is charged).
     - It keeps retrying handoff under the existing stop-and-preserve rules (X4, X5).
     - A preserved sandbox is deleted only after retention succeeds or an operator explicitly releases it.
8. **Hibernation and warm wake.** The waits design (architecture 0004 C2/C3) stays authoritative:
   - Hibernation checkpoints to central retention and ends the attempt.
   - Wake is a **new attempt** with fresh authority, which never *requires* the original sandbox.

   *Deferred by the owner on 2026-11-02; not part of the current plan.* On backends with `suspend-snapshot` (boat), the original sandbox may be kept as a **warm start**:
   1. Before archiving, the runner deletes the permit, the agent credential and the subscription-login material.
   2. The sandbox is archived, which also serves as stop proof.
   3. On wake, the continuation attempt may be placed on the same node and resume or fork from that snapshot, with new credentials delivered.

   Central retention remains the authoritative copy. If the snapshot is missing, stale or on a node that can't take the work, the continuation hydrates normally. A warm-start snapshot may outlive the original grant, because it contains no running writers and no credentials. It is deleted when the continuation starts, when the wait is cancelled, or at a configured maximum age.

### 4.1 Reconciliation and orphan control

- **Worker node restart.** The worker node reconciles against both its journal and the backend's live inventory (`list` on the backend).
  - A sandbox with a current grant is re-adopted for monitoring. The runner inside it has been reporting directly to the server, so no work is lost.
  - A sandbox with no current grant, or one the journal does not know, is stopped and destroyed after a bounded grace period. Destruction is logged with its reason.
- **Worker node restart and authority.** Grants and permits are bound to the node *and* its boot incarnation (worker spec §4.1). A new incarnation must explicitly **re-adopt** each live sandbox:
  1. It lists the sandbox's attempt and grant generation in its re-registration.
  2. The server re-binds those permits to the new boot.
  3. Until re-adoption, the server answers the runner's renewals with "awaiting node re-adoption", without extending them.
  4. If re-adoption does not happen before the renewal deadline, the runner stops and preserves.
- **Runner disappearance.** If a runner stops renewing, the grant expires under existing rules. The worker node stops the sandbox, applies the preserve-before-destroy rule in step 7, and settles the attempt as uncertain or failed. Retry is a new attempt in a new sandbox.
- **Cost safety.** Every remote sandbox carries a hard deadline equal to the grant's hard deadline plus a small margin.
  - On boat, auto-stop (TTL) is set to that deadline rather than the platform's 1-hour default or "off". Each extension is bounded by the attempt's hard deadline.
  - A periodic sweep **stops** (archives) any backend sandbox running past its deadline, then applies the preserve-before-destroy rule. No remote sandbox may keep *running*, and billing compute, indefinitely because of a lost journal entry.
  - The sweep only touches sandboxes in the node's dedicated backend scope (section 5.2).

## 5. Placement backends

All backends implement one Rust trait in aq-worker:

- create from template digest
- deliver private material
- run the runner
- inspect
- list owned sandboxes
- suspend/resume (optional)
- destroy with confirmation
- report capabilities

Backend capabilities are advertised to placement and are part of eligibility. Examples are `vm-isolation`, `nested-docker`, `suspend-snapshot` and `persistent-disk-during-attempt`.

**Proposed C1 contract extension:** add a template-presence operation, `ensure_template`, to `PlacementBackend` so admission can preflight an exact allowlisted digest through the common interface. It must confirm that the backend's materialization is present, or return a typed refusal or uncertainty; a failed check is not template availability. It does not create a sandbox or grant execution authority. C1 currently has no such trait operation, and C2's `pull_template` is backend-specific. The C1/admission owners should settle the signature and conformance coverage before adoption; this is a recommendation, not a claim that the method is implemented.

**Terminal and observation transport per sandbox:** runners talk to the server directly, so the terminal and observation long polls are held per *sandbox*, not per node. This amends architecture 0003 §5.3 ("at most one per node") and worker spec 0003 §5. The existing per-node aggregate limits still apply: subscriptions, viewers and bytes are counted server-side across all of a node's sandboxes. No poll is held for a sandbox with no subscriptions or pending work.

### 5.1 `docker-local` (desktop and any self-managed host)

- The worker node runs as a container with access to a **dedicated rootless sandbox daemon** through its Docker-compatible API on a dedicated per-user Unix socket. The selected implementation is rootless Podman ≥ 5; rootless `dockerd` is an equivalent alternative. The backend requires `/info` `SecurityOptions` to include `name=rootless`, and refuses a rootful daemon. The socket is never the host's main Docker socket.
- That daemon is used only for sandboxes. It holds only template images and sandbox containers, with its own storage and configuration separate from the host's main daemon.
- This deliberately revises the worker spec's "no host Docker socket" baseline for this node class. The worker spec already requires that projects needing extra capabilities be routed to an explicitly configured node class.
- The worker node's launch policy is fixed. Sandbox containers:
  - run only allowlisted template digests;
  - are never privileged;
  - drop all capabilities, set no-new-privileges, and retain the seccomp policy;
  - have no host bind mounts except the worker-managed per-attempt volume and declared mount aliases (worker spec §5);
  - have CPU, memory and PID limits taken from the reservation;
  - get a default bridge network with outbound access only.
- **Resource-limit prerequisite:** the host must use cgroup v2 and delegate the `cpu`, `memory` and `pids` controllers to the daemon's user. A host without that delegation is not admissible: Podman can otherwise silently drop the reservation limits. Qualification must read back effective limits (`cpu.max`, `memory.max`, `pids.max`) inside a sandbox; setting create-request fields alone is not proof of enforcement.
- **Private material:** upload an archive through the control API before the container starts (`PUT /containers/{id}/archive?path=/`). Its directory has mode `0700`, its file has mode `0600`, and both are owned by the intended runner identity. The material path is outside the work volume and every mount alias; aliases must not overlap or expose it. Material never goes in environment variables, command-line arguments or labels. These permissions provide runner/agent separation only with the distinct UID mappings required by SBX7.
- **WSL2 UID mapping:** install the `uidmap` package providing setuid `newuidmap` and `newgidmap`, and configure the daemon user's subordinate ranges in `/etc/subuid` and `/etc/subgid`. The C2 host lacked those helpers. `AQ_SANDBOX_SINGLE_UID=1` allowed diagnostic qualification with a single mapped host UID, but loses in-sandbox runner/agent UID separation. Mode `0600` does not restore that boundary. This fallback does not satisfy SBX7 or qualify agent execution requiring that separation; install the helpers and restart without the fallback before qualifying it. It is not a silent admission downgrade.
- Isolation claim: separate attempts cannot see each other's files, processes or credentials. **Local containers share the host kernel and are not claimed to be hostile-code isolation** (consistent with W6). The single-UID diagnostic run does not prove the SBX7 in-sandbox boundary.
- Nested Docker for agents is not provided by default. A template or node can advertise `nested-docker` only through an explicitly configured runtime, for example Sysbox. Projects that require it route only to such nodes.

### 5.2 `boat` (cloud, first remote backend)

boat.dev provides persistent Ubuntu VMs that include Docker, through an HTTP API (`https://boat.dev/api/v1`, bearer API keys). The API covers create, fork from a snapshot, stop-and-archive (snapshot), resume, delete, command execution, file transfer, scoped API keys, auto-stop and webhooks.

- **Template:** a boat saved snapshot built from a base VM by pulling and verifying the template's OCI image digest. The snapshot ID is recorded against the template digest as provenance.
- **Execution layout:** the runner and agent run **inside the template's OCI container** under the VM's Docker, so behaviour is identical to `docker-local`. VM root and the VM's Docker daemon belong to the sandbox host side. The agent user has no access to them, so the runner/agent separation (SBX7) still holds.
- **Create:** fork the template's saved snapshot. Size (small, default or large) is derived from the attempt's resource request.
- **Isolation:** each attempt is its own VM, which is stronger than containers. The backend advertises `vm-isolation`. `nested-docker` for agents is **not** offered initially. Exposing the VM's Docker daemon to the agent would undo the separation between the runner and agent users. It needs a separate design, for example a rootless daemon owned by the agent user.
- **Hibernation warm start:** stop-and-archive snapshots the VM. Resume uses the newest snapshot of that sandbox. Advertise `suspend-snapshot` (section 4, step 8).
- **Dedicated scope:** each `boat` worker node uses a boat organization (or account) dedicated to AQ attempt sandboxes. Template builds use a separate organization or key scope. Sandboxes are named with the node's prefix and attempt identifier. Reconciliation and sweeps act only on sandboxes in that dedicated scope that carry the node's prefix.
- **Destroy:** `DELETE /sandboxes/{id}`, followed by confirming the sandbox is gone. Policy also deletes that sandbox's retained snapshots unless the attempt is suspended.
- **Credentials:** the worker node's boat API key is a node-local secret. Use a scoped key with only sandbox-lifecycle, command and file permissions. It is never delivered into a sandbox. boat's own "AI credentials" and AI-proxy features are **not** used initially. AQ's credential handling in section 7 stays the single owner. This can be revisited as an explicit option.
- **Inbound exposure:** none. No public URLs, no SSH keys for agents, desktop streaming off.
- **Constraints to accept or qualify:**
  - EU-only regions, so there is transatlantic latency to a US VPS. Renewal, stop and preserve margins must assume an RTT of 250 ms or more. The terminal latency target (architecture 0003 §5.3, defined at ≤50 ms RTT) is not claimed on boat.
  - Account limits on active sandboxes.
  - The vendor is young, so it sits behind the backend interface and has no product-specific coupling outside the backend module.

### 5.3 `cloudflare-containers` (cloud, second remote backend, later phase)

Cloudflare Containers are started and controlled from a Cloudflare Worker through Durable Objects. They cannot be driven directly by an outside process.

- **Architecture:**
  - Deploy a small AQ-owned Cloudflare Worker ("sandbox gateway") that exposes a narrow authenticated API: create from template, deliver material, start runner, inspect, list, destroy.
  - The enrolled worker node calls this gateway outbound. A single worker node with the `cloudflare-containers` backend owns the gateway's credentials.
  - The gateway is part of the aq-worker repository and is released with it.
- **Template:** the OCI image digest, pushed to the registry Cloudflare Containers pulls from.
- **Constraints:**
  - Ephemeral disk, so step 6 handoff must complete before the sandbox ends.
  - No snapshot suspend, so continuation goes through central retention.
  - Instance size and image-size limits.
  - Outbound internet must be enabled for the sandbox to reach AQ, provider endpoints and Git remotes.
  - Nested Docker is not expected, so `nested-docker` is not advertised unless verified.
- **Acceptance:** this backend must pass the same backend conformance suite as the others (section 10). It is required before the final product gate. It is not required for the first cloud deployment.

## 6. Authority: delegated attempt permits

The worker protocol currently binds grants to one node identity and boot incarnation. A sandbox runner is a separate process, and often a separate machine, that must report for that attempt.

- When the node accepts an offer, the server issues the grant to the node as today. The node then requests a **delegated attempt permit**: a credential scoped to one attempt, one grant generation, one sandbox identity, and the grant's hard deadline.
- The runner uses the permit directly against the server for Started, renewal, observations, terminal transport, material fetch where allowed, artifact transfer and result reporting. The node is not a relay for these. This is required for remote backends, where the node is not on the data path.
- The permit cannot accept offers, register incarnations, act for other attempts or outlive the grant. Revoking the node revokes all of its permits. Revoking or cancelling the attempt revokes its permit.
- Cancellation, renewal refusal and expiry reach the runner in responses to its own outbound requests (W2/R1 unchanged). The node also observes cancellation and destroys the sandbox if the runner does not stop by the deadline.
- The permit is a published contract change, owned by aq-worker protocol per the ecosystem design, with daemon and worker fixtures.

## 7. Credentials

| Credential | Held by | Reaches sandbox? |
| --- | --- | --- |
| Node credential and the node's Cloudflare Access service token | Worker node | Never |
| Runner Access service token: a separate Access service token per worker node, used only by that node's runners to pass Cloudflare Access on the machine hostname | Runner | Yes, readable only by the runner's OS user. It only gets requests through Access; every request still needs a valid permit. Revoked together with the node. |
| Git remote credentials | Central aq-git | Never (cold fetch and publication are central) |
| Supervisor provider credentials | Central vault, supervisor host | Never |
| Backend API key (boat, Cloudflare gateway) | Worker node | Never |
| Delegated attempt permit | Runner | Yes, readable only by the runner's OS user |
| Scoped AQ agent credential (graph context, messaging) | Agent user | Yes, short-lived, per attempt. The agent's AQ client reaches the server through the runner's loopback proxy, which adds the runner Access token, so the agent never holds an Access token. |
| Provider API key | Runner's credential proxy | The key is present in the sandbox, readable only by the runner user; the agent user reaches the provider through a loopback proxy (the existing `aq-responses-transport` pattern) |
| Subscription login material (for example Codex `auth.json`, Claude credentials) | Central credential vault, leased per attempt | Yes, installed into the agent user's harness home (these CLIs must read it directly) |

Rules:

- **Separate OS users inside every sandbox:** `runner` and `agent`. Permit, proxy keys and runner state are not readable by `agent`. This is the in-sandbox boundary. It is not a claim of protection against a hostile agent on a shared-kernel container.
- **Subscription logins are leased, written back and wiped:**
  - Material is delivered through the existing private material lease (`/v1/worker/profiled/material`), extended to the delegated permit.
  - If the harness refreshes its login during the attempt, the runner returns the refreshed material to the vault before teardown, through a new lease-scoped write endpoint. A refresh-token rotation must not silently invalidate the stored copy.
  - The write-back creates a new credential revision marked as a **refresh** in the same lineage, written by the current lease holder. Renewal fencing (0002 §5) treats a refresh-lineage revision from the attempt's own lease as continuity, not rotation. An operator rotation or revocation still refuses renewal.
  - Teardown deletes the material.
- **Concurrent use is a per-credential policy:** each subscription credential profile declares `max_concurrent_leases`, with a default of 1. A value above 1 is allowed only for credentials marked `refresh_safe`, meaning the provider does not rotate refresh tokens on use. Without that mark, concurrent write-backs could invalidate each other. Placement treats a credential at its lease limit as unavailable (P7).
- **Placement policy per credential profile:** each credential profile lists the backends it may be used on. The default is every enabled backend, for API keys and subscription logins alike (section 1.6). An operator may narrow the list for a credential.
- **The agent can read subscription logins.** Harness CLIs read their login file directly, so the material sits in the agent user's harness home. The `runner`/`agent` separation does not protect it. It protects only the permit, the proxy-held API keys and runner state. The controls for subscription logins are: lease scope, one attempt per sandbox, wipe at teardown, and the J4 security review.
- Template images and boat saved snapshots must contain no credentials. The template pipeline scans the images and snapshots it produces for known credential paths and fails if any are found.

## 8. Sandbox templates

### 8.1 One template family for all harnesses

Templates vary by **project toolchain**, not by harness. Every template contains every qualified harness CLI at its pinned version. The launch manifest selects which harness runs. There is no image per harness and no harness × project image matrix.

Why:

- All four harnesses are CLI programs of modest size.
- The installed but unselected harnesses are inert: they hold no credentials, because only the selected harness's credential material is delivered (section 7).
- The axis that actually multiplies images is the toolchain.

A separate per-harness image is justified only if two qualified harnesses need conflicting system dependencies. The adapter qualification must record that before a split is allowed.

**Maintained artifacts.** Three kinds of artifact are maintained, and a template is assembled from them:

| Artifact | Contents | Maintained by | Rebuild cadence |
| --- | --- | --- | --- |
| `aq-sandbox-os` base image | OS, Git, tmux, bubblewrap (section 8.3), CA certificates, the `runner`/`agent` users | AQ, released with the worker | OS security updates (scheduled) |
| Project toolchain images | `FROM aq-sandbox-os` (or another allowlisted base, see below) plus that project's declared toolchain recipe, such as Rust, Node or Python. Toolchains may also derive from another toolchain image, for example `rust-node` from `rust`, with parents declared. | Per project, recipe in the vault | When the recipe or its parent changes |
| `aq-sandbox-runtime` bundle | Everything AQ owns inside a sandbox, under `/opt/aq`: the sandbox runner, aq-git pinned by digest, the credential and AQ loopback proxies, and every qualified harness CLI at its pinned version. It includes a private Node runtime for harnesses that need one, so no harness depends on the project's Node version. | AQ, released with the worker | Every worker or harness release |

A **template** is a project toolchain image plus the runtime bundle copied in as the final layer (`COPY --from=aq-sandbox-runtime@sha256:… /opt/aq /opt/aq`), followed by a fixed finalize step. That step checks the users, tmux, bubblewrap and Git, and refuses the build if any is missing.

The layer order is deliberate. The parts that change most often (runner and harness versions) sit on top. A harness update therefore rebuilds every template quickly, as one copied layer, instead of forcing every project to reinstall its toolchain. Adding a project means writing a toolchain recipe, not a whole image.

**Alternative base.** A general-purpose "universal" toolchain may use OpenAI's [`codex-universal`](https://github.com/openai/codex-universal) reference image (`ghcr.io/openai/codex-universal`) as its base instead of `aq-sandbox-os`. That image preinstalls Python, Node, Rust, Go, Ruby, PHP, Swift and JDK, with versions selected by `CODEX_ENV_*_VERSION`. The finalize step adds what `aq-sandbox-os` would have provided. Image size, license terms and update cadence must be checked first. It is an option; slim per-project toolchains remain the default.

**On boat,** "fork" is literal: each template has a saved snapshot in which that template's image is already pulled, and every attempt forks that snapshot (section 5.2). Rebuilding a template produces a new snapshot. Old snapshots are deleted once no active attempt, warm start or rollback target references their digest.

A template advertises the exact harness versions it contains. Selection requires that the chosen template contain the chosen harness version (P5, SBX13).

### 8.2 Template artifacts

- **Project templates:** a project template is the layers above with a declared toolchain recipe, optionally with pre-fetched repository objects.
- **Definition:** template recipes are editable non-secret definitions in the configuration vault, alongside profiles. Building produces immutable artifacts:
  - an OCI image digest;
  - per backend, a materialization record (boat snapshot ID, Cloudflare image reference) with provenance (recipe digest, base digest, build time, builder identity).
- **Resolution:** an execution profile's environment resolves to exactly one template digest at catalog activation. The launch manifest pins it (architecture 0003 §3.1). Rebuilding a template creates a new digest. Running attempts are unaffected.
- **Cost:** template builds run on an explicitly configured build node, not on ordinary attempt capacity. The local desktop is acceptable for the first deployment.
- Arbitrary images are not allowed. Only templates built by the pipeline and allowlisted by digest may launch.
- **Registry:** base templates are published with releases. A project template that bakes in pre-fetched objects from a private repository must be pushed to a private registry or package, never to the public release registry.

### 8.3 Harness configuration inside sandboxes

Each harness adapter documents how it maps the vendor's headless/container guidance onto these rules. That mapping is part of adapter qualification (H2; WRK-08, WRK-11, WRK-12, WRK-13).

**Common rules:**

- **Private config directory.** Each attempt gets its own harness configuration and credential directory in the agent user's home. Never a shared home, and never the template's.
- **Credentials are stored as files.** OS keyrings are unavailable or unreliable in sandboxes, and a keyring-held login cannot be leased or written back.
- **Non-interactive operation.** No approval prompts that would wait for a human who isn't watching. Human-facing interaction goes through AQ messaging and decisions, and the tmux session remains watchable and controllable (architecture 0003).
- **Declared filesystem confinement.** Every attempt's confinement mode is declared, tested and visible (W6). The two allowed modes are below.

**Confinement modes,** declared per execution profile and qualified per template × backend:

| Mode | Meaning | Allowed when |
| --- | --- | --- |
| `native` (preferred) | The harness's own sandbox is active inside the sandbox. This gives defence in depth: the agent's commands are confined to the workspace even inside the container. | The template contains the harness's sandbox dependencies and the backend permits them. For Codex on Linux, that means bubblewrap plus unprivileged user-namespace creation and UID mapping (see below). |
| `container` | The harness's own sandbox is disabled and the sandbox itself is the boundary. Codex calls this `danger-full-access`, which OpenAI says to use "only in a controlled environment". | Only when the execution profile explicitly allows it, the backend provides the sandbox boundary (SBX10/SBX11), the runner/agent user separation is in place (SBX7), and the attempt holds only its own credentials. Never selected as a silent fallback when `native` fails (SBX13). |

**Codex CLI,** from OpenAI's [authentication](https://developers.openai.com/codex/auth), [non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode.md) and [sandboxing](https://learn.chatgpt.com/docs/sandboxing.md) documentation:

- **Config directory and credential store.** Set `CODEX_HOME` to the attempt's private directory and `cli_auth_credentials_store = "file"`, so the login lives in `$CODEX_HOME/auth.json`.
- **ChatGPT login.** OpenAI's documented headless/CI pattern matches the lease in section 7:
  1. Put `auth.json` in place from secure storage.
  2. Run Codex, which refreshes the file in place.
  3. Save the updated file between runs.

  OpenAI calls ChatGPT-managed auth on runners an advanced option for trusted runners and says API keys are the right default for automation. It also warns not to use this workflow for public repositories that run untrusted code. AQ sandboxes run only the owner's agents on the owner's projects, which is the trusted-runner case. Projects that run untrusted contributions should restrict their credentials to API keys (section 7).
- **Getting a ChatGPT login into the vault.** Run `codex login --device-auth` in a one-off *enrollment sandbox* whose `CODEX_HOME` is captured straight into the vault, so the file is never copied off a desktop by hand. Device-code login must be enabled in the ChatGPT account's security settings.
- **API keys.** OpenAI warns against job-level `OPENAI_API_KEY`/`CODEX_API_KEY` environment variables where repository code runs. AQ goes further: Codex is configured with a custom model provider pointing at the runner's loopback Responses proxy (the existing `aq-responses-transport`). The key never reaches the agent user's environment or files.
- **Native sandbox on Linux.** Codex uses `bwrap` (bubblewrap) from `PATH`. Without it, Codex falls back to a bundled helper that needs unprivileged user-namespace creation.
  - Templates install bubblewrap.
  - On `boat`, a full VM, the `native` mode is expected to work and must be qualified.
  - On `docker-local`, C2's WSL2 probe under the fixed launch policy recorded `CapEff=0`, `NoNewPrivs=1` and `Seccomp=2`. `unshare -U true` succeeded, but `unshare -Ur id` failed when writing the UID map with `EPERM` ("Operation not permitted"). User-namespace creation alone is therefore insufficient for Codex's bubblewrap sandbox. **Codex `native` is unavailable in the qualified docker-local configuration; confinement stays `container`.** Profiles must explicitly permit `container`; a profile requiring `native` is refused. The probe does not establish which host or runtime control denied mapping. Reconsidering `native` requires a new qualification under the fixed policy, not an automatic fallback or relaxed launch policy. Shared-kernel containers are not hostile-code isolation. See section 14 for the evidence and the single-UID qualification limit.
  - On `cloudflare-containers`, user-namespace creation and UID mapping remain to be qualified independently. The docker-local probe establishes no Cloudflare result.
- **Scripted invocation.** The adapter may run `codex exec` inside the attempt's tmux session, with these options:
  - `--json` as the structured event source for observation (architecture 0004 S2);
  - `--output-schema` for the result contract;
  - `codex exec resume` only for same-attempt continuation where the adapter qualifies it.

  Codex requires a Git repository, and the attempt workspace is always an aq-git worktree, so `--skip-git-repo-check` is never needed.

**Claude Code, Gemini CLI, OpenCode:** the same table must be filled in from each vendor's current headless documentation during WRK-11/12/13. That covers:

- headless invocation;
- config directory variable;
- credential file location and refresh behaviour;
- API-key routing through a loopback proxy;
- availability of a native sandbox and its kernel requirements.

A harness whose subscription credential cannot be stored as a file, or whose API-key traffic cannot be routed through a proxy, records that limitation. Its credential profiles are restricted accordingly.

## 9. Deployment topology requirements (first production deployment)

The VPS follows the infrastructure spec, with these additions:

- **Compose services:** `postgres`, `aq-server` (dashboard bundle built into the image), `aq-graph` (pinned image digest), `aq-git` (pinned, supervised, restarted on failure, readiness-checked by aq-server), `cloudflared`, and `aq-worker-cloud` (backend `docker-local`, with its own rootless sandbox daemon and per-service resource limits). When boat is added, a second worker service, `aq-worker-boat` (backend `boat`), is enrolled as its own node; `aq-worker-cloud` stays on `docker-local`.
- **Hostnames:** the existing human hostname, behind Cloudflare Access with owner login, and machine hostname, behind Access service tokens. All workers and sandbox runners use the machine hostname. Each worker node has two service tokens, one for the node and one for its runners (section 7).
- **Supervisor host:** a supervisor host container (section 3.1) runs supervisor CLI sessions. Its memory is part of the sizing below.
- **VPS sizing:** the infrastructure spec's starting size (2 vCPU / 4 GB) was chosen for the foundation server alone. The VPS now also hosts the supervisor host and the cloud worker's agent sandboxes. Size it from measured memory for the stack plus the cloud worker's sandbox ceiling, roughly 8 vCPU / 16 GB for two concurrent Rust-building sandboxes, before purchase. Check Bluehost's sustained-CPU terms.
- **Local desktop:** the worker container plus the dedicated sandbox Docker daemon on WSL2. It must survive desktop reboot. On restart it reconciles as in section 4.1.
- **Backups** follow the infrastructure spec. Sandboxes are never backed up; central retention is the durable copy.

## 10. Requirements

| ID | Requirement |
| --- | --- |
| SBX1 | Each worker-placed agent attempt runs in its own sandbox. A sandbox never hosts more than one attempt. A hibernated sandbox may only be warm-started by that attempt's continuation, after its credentials have been wiped. |
| SBX2 | Worker nodes run no agent processes; they own capacity, journal, backend credentials, credential leases and sandbox lifecycle. |
| SBX3 | Sandbox creation is journaled before the backend call; the backend identifier is recorded; restart reconciles journal against backend inventory and destroys unknown or ungranted sandboxes after a bounded grace. |
| SBX4 | Backend-confirmed destruction, or a confirmed stop with no running processes, counts as stopped-writer proof for writers inside that sandbox. Without confirmation, resources stay protected. |
| SBX5 | Work reported finished has reached central retention. A sandbox holding unretained work is stopped and preserved, never destroyed, until retention succeeds or an operator releases it. |
| SBX6 | Runners report directly to the server under a delegated permit. The permit is preparation-scoped until the grant, then execution-scoped, and is bound to attempt, grant generation, sandbox identity, node boot and hard deadline. A new node boot must explicitly re-adopt live sandboxes. Permits cannot act beyond their scope. |
| SBX7 | Node and backend credentials never enter a sandbox; runner and agent are separate OS users; provider API keys are reached through a loopback proxy. |
| SBX8 | Subscription login material is leased per attempt, written back if refreshed, wiped at teardown, and limited by `max_concurrent_leases`. It is allowed on every enabled backend unless the operator narrows that credential's backend list. |
| SBX9 | Templates are pipeline-built, digest-pinned, credential-free and allowlisted; each backend materialization records provenance back to the image digest. |
| SBX10 | `docker-local` uses a dedicated per-user rootless Podman ≥ 5 or rootless `dockerd` daemon through its Docker-compatible API; `/info` must report `name=rootless`. Its fixed, non-privileged launch policy enforces reservation CPU, memory and PID limits through cgroup v2 user delegation; hosts lacking `cpu`, `memory` or `pids` delegation are not admissible. Private material is archived before start into a `0700` directory and `0600` file outside the work volume and mount aliases. On WSL2, `uidmap` and subordinate UID/GID ranges are prerequisites for SBX7 separation; `AQ_SANDBOX_SINGLE_UID=1` is a diagnostic fallback that loses that separation and does not meet SBX7. Shared-kernel containers do not claim hostile-code isolation. |
| SBX11 | `boat` creates sandboxes by forking a template snapshot, sets auto-stop to the grant's hard deadline, supports snapshot suspend for the same attempt, and confirms deletion. |
| SBX12 | `cloudflare-containers` operates through an AQ-owned gateway Worker, completes handoff before ephemeral teardown, and passes the same conformance suite. |
| SBX13 | Backend capabilities (isolation level, nested Docker, suspend, limits) are advertised and used in eligibility; unsupported requirements refuse, never degrade silently. |
| SBX14 | Placement preference across nodes is explicit and explainable; the default prefers `docker-local`, then `boat`, then `cloudflare-containers`. |
| SBX15 | Every remote sandbox has a hard deadline and is stopped by a sweep if it runs past it. Sweeps act only within the node's dedicated backend scope. Spend and concurrency ceilings per node refuse further offers when reached. |
| SBX16 | Terminal and observation polls are held per sandbox, with the existing per-node aggregate limits enforced server-side across a node's sandboxes. |
| SBX17 | Sandboxes never hold Git remote, node, backend or supervisor credentials. Cold fetch and publication are central. The agent reaches AQ and providers only through the runner's loopback proxies. |
| SBX18 | Templates vary by toolchain, not harness. Each template is a toolchain image (normally `FROM aq-sandbox-os`) plus the `aq-sandbox-runtime` bundle as its final layer, carrying all qualified harness CLIs at pinned versions. Templates advertise those versions, and selection requires a match. |
| SBX19 | Each attempt uses a private, file-based harness credential directory and a declared confinement mode (`native` or `container`); `container` mode requires explicit profile permission and backend isolation, and is never a silent fallback. |
| DEP1 | The VPS runs the full server stack (server with built-in dashboard, PostgreSQL, aq-graph, supervised aq-git, cloudflared) from pinned images through Compose. |
| DEP2 | The cloud worker runs on the VPS as a separate enrolled worker using the public machine hostname through Access, with `docker-local` sandboxes on the VPS. `boat` is added later as a second enrolled VPS worker node (`aq-worker-boat`) under the same rules; each node keeps exactly one backend. |
| DEP3 | A local desktop worker runs as a container on WSL2 with a dedicated rootless sandbox daemon, survives reboot and reconciles on restart. |
| DEP5 | Central supervisor sessions run in a supervisor host container on the VPS under central session identity, separate from worker nodes. |
| DEP4 | Releases include the worker image, sandbox base template and runner versions as manifest-pinned artifacts (product release manifest, ecosystem §5). |

## 11. Acceptance scenarios

| Scenario | Required evidence |
| --- | --- |
| Backend conformance | One suite run against every backend: create, provision, run fixture agent, observe, hand off, destroy with confirmation; create-then-crash before recording; crash after create; unknown sandbox in inventory; destroy failure keeps resources protected. |
| Real agent on each deployed backend | Codex CLI with an API key and with a ChatGPT login on desktop `docker-local`, VPS `docker-local` and `boat`; graph context, edit, handoff, version-checked outcome. |
| Isolation | Two concurrent attempts on one node cannot read each other's workspace, credentials or processes; the agent user cannot read the permit, proxy key or runner state. |
| Harness confinement | For each template × backend: Codex `native` mode either works (writes outside the workspace are refused inside the sandbox) or the backend is recorded as `container`-only; a profile requiring `native` is refused on a `container`-only backend. |
| Credential refresh | A subscription login refreshed during an attempt is written back; the next attempt succeeds with the stored copy. A second concurrent lease beyond `max_concurrent_leases` is refused. |
| Recovery | Kill the worker node during a remote attempt: the runner keeps reporting, and on restart the sandbox is re-adopted. Kill the runner: the grant expires, the sandbox is destroyed, and a retry starts in a new sandbox. Lose a journal entry: the orphan sweep destroys the sandbox before its deadline margin. |
| Hibernation and wake | Hibernate a waiting attempt; wake starts a new attempt with fresh authority. On boat, from the credential-wiped snapshot, with the workspace intact; with the snapshot deleted, from central retention. In both cases there are no duplicate effects, and no credentials remain in the archived snapshot. |
| Preserve before destroy | Partition a remote sandbox from the server mid-attempt: the sandbox is stopped and preserved, not destroyed, and handoff completes after reconnection. Operator release is required to discard it. |
| Node re-adoption | Restart a worker node during an attempt: renewals report "awaiting re-adoption" until the new boot re-adopts the sandbox. Without re-adoption, the runner stops and preserves at the renewal deadline. |
| Cost control | A sandbox exceeding its hard deadline is destroyed; a node at its spend or concurrency ceiling refuses with a typed reason. |
| Deployment | Fresh VPS install brings up the full stack; local and cloud workers enroll; an owner can enqueue a task from the dashboard, talk to the supervisor, and watch the attempt in the terminal view. |

## 12. Changes to existing documents

These documents are updated to reference this design:

- **Ecosystem and releases:** runtime boundaries, ownership table and deployment section.
- **Infrastructure foundation:** "disposable execution sandboxes" moves from deferred to specified here.
- **Worker framework:**
  - Worker nodes no longer host agent processes directly; the "N agent attempts" topology becomes "N sandboxes".
  - The "no host Docker socket" baseline gains the `docker-local` node-class exception.
  - The "stronger per-attempt isolation can later be added" note is fulfilled.
- **Profiles, harnesses and terminals:** tmux and the guardian run inside the sandbox; environments resolve to template digests. The "at most one terminal long poll per node" limit becomes per sandbox, with per-node aggregates (SBX16).
- **Worker profiled tmux sessions (0003 §5):** same per-sandbox polling amendment.

The existing guarantees on grants, stop proof, retention, tmux, terminals and outbound-only networking are unchanged. This design changes *where* they are enforced.

## 13. Left to the implementer

- The Rust shape of the backend trait, the permit wire format and the gateway API, within the requirements above.
- The sandbox daemon choice is resolved by C2: rootless Podman ≥ 5 via its Docker-compatible API on a dedicated per-user socket; rootless `dockerd` is an equivalent alternative. The backend checks `/info` `SecurityOptions` for `name=rootless`. Socket layout and service installation must preserve the dedicated scope and the cgroup/UID-mapping prerequisites in section 5.1.
- Template build tooling.
- Exact grace periods, sweep intervals and deadline margins. They must be bounded and configurable.

## 14. To verify during implementation

The C2 docker-local qualification on 2026-11-02 used rootless Podman 5.8.8 (Docker-compatible API 1.44) on WSL2 kernel `6.18.33.2-microsoft-standard-WSL2`. Its confinement probe is recorded in `aq-worker` task `calm-delta-18.2`, `docs/implementation/C2-evidence/confinement-probe.log`; the findings and setup are in `docs/implementation/C2.md`. Under `CapEff=0`, `NoNewPrivs=1`, `Seccomp=2`, `unshare -U true` returned 0, while `unshare -Ur id` failed writing the UID map with `EPERM`. This configuration does not support Codex `native`; `container` remains the explicit confinement mode. The run used `AQ_SANDBOX_SINGLE_UID=1` because setuid UID-mapping helpers were missing, so it did not qualify SBX7 runner/agent separation. Install `uidmap` and requalify with distinct mappings; any future native-mode claim requires a fresh probe under the fixed policy. Do not infer results for other hosts or backends from this run.

Evidence is pinned to aq-worker commit `267e9bdf094a93fcf77752ca53e6a551fa00e6a9`: [C2 findings and setup](https://github.com/ElectricJack/aq-worker/blob/267e9bdf094a93fcf77752ca53e6a551fa00e6a9/docs/implementation/C2.md) and [confinement probe](https://github.com/ElectricJack/aq-worker/blob/267e9bdf094a93fcf77752ca53e6a551fa00e6a9/docs/implementation/C2-evidence/confinement-probe.log). The [design patch handoff](https://github.com/ElectricJack/aq-worker/blob/20791e356df9f5d5b45031ca14304672b071d500/docs/implementation/C2-0005-patch.md) records their SHA-256 hashes and the verification limits. This documentation update does not rerun or extend that qualification.

These platform facts remain to be verified:

- **boat:**
  - API details for create, fork, delete and auto-stop.
  - Scoped-key permissions.
  - Snapshot and resume time.
  - Outbound firewall controls.
  - Account sandbox limits.
- **Cloudflare Containers:**
  - Instance and image limits.
  - Outbound network configuration.
  - Registry requirements.
  - Whether nested Docker is possible.
- **Provider terms** for running each subscription login in remote sandboxes.
- **Confinement follow-up:** qualify runner/agent separation on WSL2 after installing `uidmap` and disabling the single-UID fallback. Codex `native` remains unavailable for the C2 docker-local configuration; any proposed change needs both user-namespace creation and UID-mapping evidence under the fixed policy. Qualify bubblewrap independently in Cloudflare Containers.
- **`codex-universal`:** image size, license and suitability as a universal toolchain base.
- **VPS memory** for the full stack.

Record the findings in the backend qualification evidence. A finding that contradicts a requirement here requires a design revision, not a silent workaround.

## 15. Decisions made during design review

These calls were made by the designer while resolving a builder review on 2026-11-02. Each one follows rules already in the referenced documents. The product owner should check them.

1. **Supervisors** run centrally in a supervisor host container on the VPS, not in sandboxes. This follows W1 and architecture 0003 §4.
2. **Cloudflare Access for runners:** each worker node has a separate per-node runner service token. The agent reaches AQ through the runner's proxy.
3. **Hibernation:** wake is always a new attempt (architecture 0004). A boat snapshot is only an optional warm start, archived with its credentials wiped first.
4. **Preserve before destroy:** sandboxes with unretained work are stopped and kept, not destroyed. Deadline sweeps stop sandboxes; they don't delete them.
5. **Permits** have a preparation phase, so a sandbox exists before the grant. Permits are boot-bound, and the node re-adopts its sandboxes after a restart.
6. **Subscription refresh write-back** is a refresh-lineage revision that renewal fencing accepts. Concurrent leases above 1 require `refresh_safe`.
7. **Terminal and observation polls** are per sandbox, with the per-node limits counted on the server.
8. **boat:** a dedicated organization for attempts, a separate scope for builds, and name-prefix scoping for sweeps. Runner and agent run in the OCI container inside the VM. Nested Docker is not offered initially.
9. **Git remote credentials** stay central. Templates that contain private source go only to a private registry.
10. **Templates vary by toolchain, not harness.** AQ maintains an `aq-sandbox-os` base and an `aq-sandbox-runtime` bundle (runner, aq-git, proxies, all harnesses, private Node). Projects maintain only a toolchain recipe `FROM aq-sandbox-os`. The bundle is copied in last, so harness updates are cheap.
11. **Harness confinement:** the harness's own sandbox (`native`) is preferred inside our sandbox. `container` mode, with the harness sandbox off, is explicit only. Codex follows OpenAI's documented headless patterns: file credential store, device-code enrollment, refresh written back, and API keys through a proxy.

Decisions after the independent review and owner answers (2026-11-02):

12. **Cloud worker rollout:** the VPS cloud worker runs `docker-local` sandboxes first. boat follows as a second enrolled VPS worker node, `aq-worker-boat`, because each worker node has exactly one backend; `aq-worker-cloud` stays on `docker-local` (program plan revision log, 2026-11-02). Cloudflare Containers is built after the core product (owner). The desktop worker itself runs as a container on WSL2 (owner).
13. **Subscription logins are allowed on every backend** (owner, personal deployment). The design states plainly that the agent can read them.
14. **`docker-local` stays at `container` confinement.** C2 showed that the fixed policy permits user-namespace creation but refuses the UID-map write Codex's bubblewrap sandbox needs (sections 8.3 and 14). Codex `native` is unavailable in that configuration; shared-kernel containers are not hostile-code isolation.
15. **boat warm-start hibernation is deferred.** Hibernation uses checkpoint-to-retention only (owner deferral).
