# AQ v2 build: supervision log

This is the operator log for the v2 build running in AQ v1. Projects:
- aq-platform (the aq-server repository)
- aq-worker
- aq-dashboard
- aq-graph
- aq-git
- aq-playbooks

The newest entries are at the bottom. Lessons drawn from these entries live in the [retrospective](2026-11-02-v1-hosting-retrospective.md). The task-key mapping is in `docs/superpowers/plans/2026-11-02-v2-full-application-v1-ids.json`.

Each entry records what was observed, the action taken, and anything waiting on the owner.

## 2026-11-02 (Pacific)

- **v1 started.** It had been stopped on purpose on 10-30. Postgres recovered from an unclean shutdown. The owner paused all other projects except aq-design-tour, which is still active.
- **aq-platform created.** aq-server is its primary repository. Five workspace kinds and a read-only `platform-ref` were added. The default policy was applied (two project playbook copies are pending review, and routing preferences are unconfigured).
- **Plan imported:** 88 tasks, 197 edges, an MS0 release gate. Two automatic onboarding tasks (graft hooks) ran.
- **Parallelization review applied:**
  - The release gate was removed.
  - B6a and C5a were split.
  - Edges were fixed and priorities set.
  - The aq-platform cap was raised to 8, the standard-high Codex pool to 6 and the operator Codex pool to 2.
- **Self-contained descriptions written for all 91 tasks.** D0 was added (aq-git GitHub credentials). The footer forbids reading whole documents and doing out-of-scope work.
- **Owner decision: subscription logins, no API keys.** I0, I4a and I4b were requalified on the ChatGPT login, with refresh write-back. ElectricJack/aq-scratch was created as the public scratch repository.
- **Routing failures for non-primary repositories** (`no_candidates / workspace_requirement`). Task-lifecycle Codex profiles were added (`standard-high-codex-task` and `deep-high-codex-task`, 4 agents each).
- **Worktree authorization only allowed the primary repository.** A v1 patch was tried in the live checkout and reverted within about 2 minutes, because v1's rules forbid editing that checkout. The project was then split: aq-worker (cap 4, later raised to 6), aq-dashboard (3), aq-graph (2), aq-git (2) and aq-playbooks (2). 47 tasks moved. Each project has a `platform-ref` symlink directory. One daemon restart.
- **Owner decisions:**
  - A 15-minute monitor loop.
  - The deep-high Claude pool enabled with max 4.
- **Recovery holds released after diagnosis** (session churn during the routing failures): A4 and C5a1 (C5a1's max retries raised to 6).
- **Mis-filed cross-repository follow-ups re-filed:**
  - B1's aq-server pin became `stark-horizon-12` in aq-platform; B2 waits on it.
  - D4's catalog pin was folded into D5.
- **Footer additions:** the `CROSS-REPO FOLLOW-UP` comment convention, and filing follow-ups as `--integration-mode direct`.
- **Four owner decisions:**
  - Merge PRs for the owner.
  - I0 may use the owner's login and act as the settlement stand-in.
  - Reviews are delegated to the supervisor in all six projects.
  - Agents publish releases in-task.
- **Applied:**
  - 30 PRs merged across the repositories (5 older conflicting PRs left open; their tasks are completed and nothing depends on them).
  - I0 reopened with the authorization.
  - Four reviews delegated.
  - E2 and A3a released to publish. A3a published aq-git and completed.
- **Mistake:** a local `git stash` pop conflict pushed conflict markers into the importer footer on aq-server main. Fixed in `ab31460` within minutes. v1's task descriptions were never affected.
- **Owner decisions:**
  - Agents may record owner attestation from observed facts (sharp-harbor-18).
  - The credential-custody design was approved (fresh-delta-13).
- `quick-glacier-78` was closed as obsolete; its work was already done in `crisp-beacon-59`.
- **I0 now depends on `crisp-beacon-59` and `sharp-harbor-18`.**
- **No project supervisors run for the six projects; only the global supervisor does.** It was asked directly to decide the delegated reviews, and it returned changes requested on B1, C5a1 and D10.
- **The review flow creates its own "Revise …" tasks.**
  - My duplicate B1 follow-up (`steady-impact-69`) was closed.
  - C5a1 and D10 now wait on their revision tasks.
- **B1's PR #4 was superseded by #12,** which carried its commits to main. The PR-merge gate resolved through the sweep. B6a1 was released.
- **Status at about 18:15:** 18 of 91 plan tasks (about 20%) and 47 follow-ups completed. Postgres is CPU-bound on the admission query, which limits scheduler throughput.
- **Waiting on the owner:** nothing open.
- **18:19 check.**
  - B6a1 is running again, with all three review revisions in flight (B1, C5a1, D10).
  - Filed G7's cross-repo follow-up as `keen-current-13` in aq-platform: additive results-feedback read fields.
  - Merged aq-worker #13 (runner renewal validation).
  - `sharp-harbor-18` (blocks I0) is in v1's automatic retry after `slot_reset_failed: branch holder reservation is not clean`; the standard-high-codex pool is at 5 of 6 busy. Re-check next pass.
- **18:34 check.**
  - The B1 server pin (`stark-horizon-12`) and B1's review revision completed. B2 started. `sharp-harbor-18` started after its automatic slot retry.
  - B5a's cross-repo requirement (registration-v2 and initial DelegatedPermit issuance routes, PermitProvider, mapping controller refusals into the transport) was routed to B2 as a comment and live message, because it is in B2's scope.
  - Merged aq-worker #14 (desktop ChatGPT login reconcile before cleanup; fresh-delta-13) and aq-playbooks #3 (D10 review revision 2 evidence).
- **19:27 check.**
  - **Owner approved a bounded qualification method for `sharp-harbor-18`:**
    - gpt-6.1-sol, reasoning high;
    - at most 3 real calls to measure counting and capacity;
    - seccomp=unconfined only for the local I0/E0 container;
    - credential profile `owner-chatgpt-codex`, desktop worker only;
    - catalog revision 2 for aq-scratch.

    Released.
  - Filed B2's contract prerequisite as `nimble-dune-30` (aq-worker): permit handoff, Prepared and mail scopes. B2 and B6a2 now depend on it.
  - Told G4 its base branch is main (it was waiting on a "base-branch decision"). Its dashboard consumer requirements went to G5.
- **19:29 check.**
  - The supervisor filed B1b (`agile-delta-29`) for the same aq-admission extension as my `nimble-dune-30`. B1b is correctly sequenced behind B1's round-2 review (`smart-grove-55`). I closed mine as a duplicate and moved the B2 and B6a2 dependencies onto B1b.
  - Raised `sharp-harbor-18` (on the MS0 path) to high priority; aq-worker is at its cap of 6.
  - Merged aq-server #20 (C5a1 revision 2), aq-worker #16 (C2 handoff docs) and aq-worker #17 (heartbeat diagnostics).
  - C3b completed; C3c (template publish) and G5 (Overview) started.
- **19:45 owner check-in.**
  - **My monitor had been missing agents' "Blocked on …" messages to the owner's dashboard inbox.** The inbox query filtered them out. The monitor now reads them directly.
  - **Owner approved GHCR publication:** `docker login ghcr.io` using the owner's gh token (write:packages), with public packages under ghcr.io/electricjack. C3c was released.
  - Asked the supervisor again to decide six delegated reviews (B1 round 2, B5a, C5a1, C2, J7, a flakiness investigation), and flagged D10's inconsistent round-3 changes-requested.
  - The 12 imported-policy-playbook reviews are owner-only but non-blocking; system defaults apply.
