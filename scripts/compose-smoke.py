#!/usr/bin/env python3
"""Disposable E0 acceptance: real Compose peers and observed crash recovery."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import uuid

spec = importlib.util.spec_from_file_location("dev_stack", Path(__file__).with_name("dev-stack.py"))
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)


def wait_for(check, description, seconds=120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("timed out: " + description)


def compose_dev_reaches_ready_with_graph_and_git():
    with tempfile.TemporaryDirectory(prefix="aq-e0-") as temp:
        directory = Path(temp)
        if "AQ_SERVER_IMAGE" not in os.environ:
            os.environ["AQ_SERVER_IMAGE"] = "aq-e0-server:" + uuid.uuid4().hex[:10]
        os.environ.update(AQ_DEV_STATE_DIR=str(directory / "state"), AQ_SECRETS_DIR=str(directory / "secrets"),
            COMPOSE_PROJECT_NAME="aq-e0-" + uuid.uuid4().hex[:10], AQ_DEV_ADMIN_PORT="0", AQ_DEV_WORKER_PORT="0",
            AQ_DEV_GRAPH_PORT="0")
        os.environ.pop("AQ_VAULT_DIR", None)
        env = dev.prepare()
        config = json.loads(dev.compose(env, "config", "--format", "json"))
        for service in ("postgres", "server", "aq-graph", "aq-git"):
            assert service in config["services"], "missing development service: " + service
            assert config["services"][service].get("healthcheck"), "missing healthcheck: " + service
        assert "cloudflared" not in config["services"], "tunnel enabled by default"
        started = False
        evidence = dict(test="compose_dev_reaches_ready_with_graph_and_git")
        try:
            if os.environ.get("AQ_DEV_SMOKE_NO_BUILD") != "1":
                # Preloaded immutable Graph/Git images may be selected explicitly.
                builds = ["server"]
                if not os.environ.get("AQ_DEV_GRAPH_IMAGE"):
                    builds.append("aq-graph")
                if not os.environ.get("AQ_DEV_GIT_IMAGE"):
                    builds.append("aq-git")
                dev.compose(env, "build", *builds, capture=False)
            started = True
            dev.compose(env, "up", "-d", "--no-build", "postgres", "graph-db-init", "aq-graph", "aq-git", "server")

            def healthy():
                rows = [json.loads(line) for line in dev.compose(env, "ps", "--format", "json").splitlines() if line]
                return rows if all(any(r["Service"] == service and r.get("Health") == "healthy" for r in rows)
                                   for service in ("postgres", "server", "aq-graph", "aq-git")) else None

            rows = wait_for(healthy, "all four services healthy")
            evidence["images"] = {r["Service"]: dev.compose(env, "images", "-q", r["Service"]).strip() for r in rows}
            def readiness():
                # /readyz is deliberately accessible only from inside the server.
                result = subprocess.run(["docker", "compose", "-f", str(dev.ROOT / "compose.yaml"), "-f",
                    str(dev.ROOT / "compose.dev.yaml"), "exec", "-T", "server", "aq-server", "readiness", "--vault", "/vault"],
                    env=dict(os.environ, **env), capture_output=True, text=True, timeout=10)
                try:
                    return result.returncode, json.loads(result.stdout)
                except json.JSONDecodeError:
                    return result.returncode, {}

            code, ready = readiness()
            assert code == 0 and ready["dependencies"] == {"graph": "ready", "aq_git": "ready"}, ready
            evidence["initial_readiness"] = ready
            def port(service, internal):
                return dev.compose(env, "port", service, str(internal)).strip().rsplit(":", 1)[1]
            os.environ["AQ_DEV_GRAPH_URL"] = "http://127.0.0.1:" + port("aq-graph", 8741)
            os.environ["AQ_DEV_SERVER_URL"] = "http://127.0.0.1:" + port("server", 8080)
            os.environ["AQ_DEV_WORKER_URL"] = "http://127.0.0.1:" + port("server", 8081)
            # Only our disposable local repository is made available to Git.
            fixture = directory / "state/fixture"
            fixture.mkdir(exist_ok=True)
            subprocess.run(["git", "init", "--bare", "--initial-branch=main", str(fixture / "scratch.git")], check=True, capture_output=True)
            os.environ.update(DEV_SCRATCH_REPO_URL="/fixture/scratch.git", DEV_SCRATCH_BASE_BRANCH="main", DEV_SCRATCH_CREDENTIAL_REF="none")
            seed = dev.seed(local_fixture=True)
            assert dev.seed(local_fixture=True) == seed, "seed replay changed identities"
            os.environ["DEV_SCRATCH_BASE_BRANCH"] = "changed-input"
            try:
                dev.seed(local_fixture=True)
            except ValueError as error:
                assert "designation changed" in str(error)
            else:
                raise AssertionError("changed designation replaced the existing project")
            finally:
                os.environ["DEV_SCRATCH_BASE_BRANCH"] = "main"
            project = dev.http(os.environ["AQ_DEV_SERVER_URL"] + "/api/projects/" + dev.PROJECT)
            assert project["revision"] == 1, project
            assert project["binding"]["allowed_node_ids"] == [seed["node_id"]]
            repository = dev.git_request(directory / "state", "repo.show")
            assert repository["remote_url"] == "/fixture/scratch.git"
            assert repository["allowed_refs"] == ["refs/heads/main"]
            assert len(dev.http(os.environ["AQ_DEV_SERVER_URL"] + "/api/workers")) == 1
            node = dev.http(os.environ["AQ_DEV_GRAPH_URL"] + "/v1/workspaces/" + dev.WORKSPACE + "/nodes/" + dev.TASK,
                            token=(directory / "secrets/graph_token").read_text())
            assert node["id"] == dev.TASK and node["kind"] == "task", node
            evidence["seed"] = seed
            before = (directory / "state/git-run/child.pid").read_text().strip()
            killed_at = time.monotonic()
            # aq-git forks a guarded engine. Kill serve, leaving its guard to
            # reap descendants and mark state clean before supervised restart.
            evidence["killed_engine_pid"] = dev.compose(env, "exec", "-T", "aq-git", "sh", "-ec",
                'guard=$(cat /git-run/child.pid); children=$(cat /proc/$guard/task/$guard/children); engine=""; for pid in $children; do if [ "$(cat /proc/$pid/comm)" = aq-git ]; then test -z "$engine"; engine=$pid; fi; done; test -n "$engine"; kill -KILL "$engine"; printf "%s\\n" "$engine"').strip()
            attempts = []
            def degraded():
                code, report = readiness()
                attempts.append(dict(seconds=round(time.monotonic() - killed_at, 2), code=code, report=report,
                    pid=(directory / "state/git-run/child.pid").read_text().strip()))
                return report if code != 0 and report.get("dependencies", {}).get("aq_git") == "degraded" else None
            try:
                evidence["after_git_kill"] = wait_for(degraded, "Git degradation after killing serve", seconds=10)
            except AssertionError:
                print(json.dumps(dict(git_kill_before_pid=before, attempts=attempts)), flush=True)
                raise
            def recovered():
                code, report = readiness()
                after = (directory / "state/git-run/child.pid").read_text().strip()
                return report if after != before and code == 0 and report.get("dependencies", {}).get("aq_git") == "ready" else None
            evidence["after_git_restart"] = wait_for(recovered, "new Git process and ready report")
            dev.compose(env, "stop", "aq-graph")
            code, graph_down = readiness()
            assert code != 0 and graph_down["dependencies"]["graph"] == "degraded", graph_down
            dev.compose(env, "start", "aq-graph")
            wait_for(lambda: readiness()[0] == 0 and healthy(), "graph recovery")
            evidence["graph_outage_degraded"] = graph_down
        finally:
            if started:
                dev.compose(env, "down", "--volumes", "--remove-orphans")
                assert not dev.compose(env, "ps", "-aq").strip(), "owned containers survived cleanup"
        evidence["cleanup"] = "owned Compose containers and volumes removed"
        print(json.dumps(evidence, sort_keys=True))
        print("PASS compose_dev_reaches_ready_with_graph_and_git")


if __name__ == "__main__":
    if sys.argv[1:] != ["dev"]:
        raise SystemExit("usage: scripts/compose-smoke.sh dev")
    compose_dev_reaches_ready_with_graph_and_git()
