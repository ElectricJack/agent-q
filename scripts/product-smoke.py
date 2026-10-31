#!/usr/bin/env python3
"""Disposable product assembly; communicate through public HTTP APIs only."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("worker_candidate", ROOT / "scripts/check-worker-candidate.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def utc():
    return datetime.now(timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


class Fixture:
    def __init__(self, execution):
        self.execution = execution
        self.project = "aq-product-" + uuid.uuid4().hex[:12]
        self.directory = tempfile.TemporaryDirectory(prefix=self.project + "-")
        self.temp = Path(self.directory.name)
        evidence = os.environ.get("AQ_SMOKE_EVIDENCE_DIR")
        self.logs = Path(evidence).resolve() if evidence else Path(tempfile.mkdtemp(prefix=self.project + "-evidence-"))
        self.logs.mkdir(parents=True, exist_ok=True)
        self.env = os.environ.copy()
        self.commands = []
        self.images = []
        self.runtime = None
        self.dc = []
        self.record = {"fixture": "execution" if execution else "foundation", "project": self.project,
                       "started_at": utc(), "commands": self.commands, "scope": "local candidate; no hosted acceptance"}

    def say(self, value):
        print(f"smoke: {value}", flush=True)

    def run(self, command, *, name, input=None, check=True, timeout=240, secret_output=False):
        start = utc()
        result = subprocess.run(command, cwd=ROOT, env=self.env, input=input, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        self.commands.append({"command": command, "started_at": start, "finished_at": utc(),
                              "exit_code": result.returncode, "log": None if secret_output else name + ".log"})
        if not secret_output:
            (self.logs / (name + ".log")).write_text(result.stdout)
        if check and result.returncode:
            if not secret_output:
                print("\n".join(result.stdout.splitlines()[-35:]), file=sys.stderr)
            raise RuntimeError(f"{name} exited {result.returncode}")
        return result.stdout

    def compose(self, *args, name, **kwargs):
        return self.run([*self.dc, *args], name=name, **kwargs)

    def poll(self, description, call, timeout=90):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                value = call()
                if value:
                    return value
            except (OSError, ValueError, urllib.error.URLError, RuntimeError):
                pass
            time.sleep(0.5)
        raise RuntimeError(f"timed out waiting for {description}")

    def http(self, path, body=None, *, machine=False, credential=None, expected=200):
        url = (self.worker if machine else self.admin) + path
        headers = {"X-AQ-Request": "1"}
        if credential:
            headers["Authorization"] = "Bearer " + credential
        data = None if body is None else canonical(body)
        if data is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(url, data, headers)
        try:
            with urllib.request.urlopen(request, timeout=5) as response:
                status, raw = response.status, response.read()
        except urllib.error.HTTPError as error:
            status, raw = error.code, error.read()
        if status != expected:
            raise RuntimeError(f"HTTP {path}: got {status}, expected {expected}")
        return json.loads(raw) if raw else None

    def address(self):
        self.admin = "http://" + self.compose("port", "server", "8080", name="admin-address").strip()
        self.worker = "http://" + self.compose("port", "server", "8081", name="worker-address").strip()

    def worker_row(self):
        rows = [row for row in self.http("/api/workers") if row["name"] == "smoke"]
        return rows[0] if len(rows) == 1 else None

    def config(self):
        # The disposable node credential is kept in memory and never in evidence.
        text = self.compose("exec", "-T", "worker", "cat",
                            "/config/agent-q/worker.toml", name="private-config-read", secret_output=True)
        return tomllib.loads(text)

    def setup(self):
        official = os.environ.get("AQ_SMOKE_OFFICIAL") == "1"
        candidate = checker.candidate(official)
        self.record["worker_candidate"] = candidate
        server = os.environ.get("AQ_SMOKE_SERVER_IMAGE")
        worker = os.environ.get("AQ_SMOKE_WORKER_IMAGE")
        if bool(server) != bool(worker):
            raise ValueError("set both AQ_SMOKE_SERVER_IMAGE and AQ_SMOKE_WORKER_IMAGE, or neither")
        if official and (not server or any(not re.fullmatch(r"[a-z0-9][a-z0-9./:_-]*@sha256:[0-9a-f]{64}", image)
                                          for image in (server, worker))):
            raise ValueError("official acceptance requires digest-pinned server and worker images")
        snapshot = self.temp / "source"
        snapshot.mkdir()
        for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Dockerfile", "Dockerfile.worker",
                     ".dockerignore", "compose.yaml"]:
            shutil.copy2(ROOT / name, snapshot / name)
        for name in ["crates", "vendor"]:
            shutil.copytree(ROOT / name, snapshot / name)
        inventory = {str(p.relative_to(snapshot)): sha(p.read_bytes()) for p in sorted(snapshot.rglob("*")) if p.is_file()}
        self.record["source_inventory"] = inventory
        self.record["source_tree_sha256"] = sha(canonical(inventory))
        self.record["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        self.record["working_tree_dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))
        state = re.search(r"pub const STATE_FORMAT_VERSION: u32 = (\d+);", (snapshot / "crates/aq-server/src/lib.rs").read_text()).group(1)
        if not server:
            server, worker = self.project + "-server:candidate", self.project + "-worker:candidate"
            self.images = [server, worker]
            revision = "dev-" + self.record["source_tree_sha256"][:24]
            self.say("build server and pinned standalone worker from a frozen source snapshot")
            self.run(["docker", "build", "--target", "server", "--build-arg", "AQ_SOURCE_REVISION=" + revision,
                      "--build-arg", "AQ_STATE_FORMAT=" + state, "-t", server, str(snapshot)], name="build-server", timeout=1800)
            self.run(["docker", "build", "-f", str(snapshot / "Dockerfile.worker"), "--target", "worker",
                      "--build-arg", "AQ_SOURCE_REVISION=" + candidate["source_revision"],
                      "--build-arg", "AQ_WORKER_ARCHIVE_SHA256=" + candidate["archive_sha256"],
                      "--build-arg", "AQ_VERSION=" + candidate["version"], "--build-arg", "AQ_STATE_FORMAT=" + state,
                      "-t", worker, str(snapshot)], name="build-worker", timeout=1800)
        else:
            for role, image in [("server", server), ("worker", worker)]:
                if subprocess.run(["docker", "image", "inspect", image], stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL).returncode:
                    self.run(["docker", "pull", image], name="pull-" + role, timeout=600)
        self.record["images"] = json.loads(self.run(["docker", "image", "inspect", server, worker], name="image-identities"))
        embedded = json.loads(self.run(["docker", "run", "--rm", "--entrypoint", "cat", worker,
                                        "/usr/share/agent-q/worker-candidate.json"], name="worker-provenance"))
        if embedded != candidate:
            raise ValueError("worker image provenance differs from selected immutable candidate")
        version = self.run(["docker", "run", "--rm", worker, "--version"], name="worker-version").strip()
        if version != "aq-worker " + candidate["version"]:
            raise ValueError("worker executable version differs from selected candidate")
        help_text = self.run(["docker", "run", "--rm", worker, "run-execution", "--help"], name="worker-execution-cli")
        if "--drop-acceptance-response-once" not in help_text or "--drop-report-response-once" not in help_text:
            raise ValueError("worker executable lacks the selected execution candidate interface")
        self.record["worker_binary_sha256"] = self.run([
            "docker", "run", "--rm", "--entrypoint", "sha256sum", worker,
            "/usr/local/bin/aq-worker", "/usr/local/bin/aq-fixture-agent"], name="worker-binary-identities")
        for name in ["vault", "secrets", "spool/requests", "spool/status"]:
            (self.temp / name).mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "vault.example/config.toml", self.temp / "vault/config.toml")
        (self.temp / "vault").chmod(0o755)
        (self.temp / "secrets/postgres_password").write_text(secrets.token_hex(24))
        (self.temp / "secrets/tunnel_token").write_text("unused-disposable-fixture")
        for secret in (self.temp / "secrets").iterdir():
            secret.chmod(0o644)
        self.env.update(AQ_SERVER_IMAGE=server, AQ_WORKER_IMAGE=worker, AQ_VAULT_DIR=str(self.temp / "vault"),
                        AQ_SECRETS_DIR=str(self.temp / "secrets"), AQ_UPDATER_SPOOL=str(self.temp / "spool"))
        overlay = {"services": {
            "server": {"environment": {"AQ_DEPLOYMENT": "development"}, "ports": ["127.0.0.1::8080", "127.0.0.1::8081"]},
            "cloudflared": {"profiles": ["tunnel"]},
            "worker": {"image": worker, "networks": ["edge"], "volumes": ["worker_config:/config", "worker_state:/state"],
                       "read_only": True, "cap_drop": ["ALL"], "security_opt": ["no-new-privileges:true"]}},
            "volumes": {"worker_config": {}, "worker_state": {}}}
        file = self.temp / "compose.smoke.json"
        file.write_text(json.dumps(overlay))
        self.dc = ["docker", "compose", "-p", self.project, "-f", str(snapshot / "compose.yaml"), "-f", str(file)]
        self.compose("up", "-d", "--no-build", "--wait", "--wait-timeout", "120", "server", name="start-server")
        self.address()
        self.poll("admin health", lambda: self.http("/healthz"))
        self.record["postgres_image"] = self.compose("images", "--format", "json", name="compose-images")

    def foundation(self):
        self.say("enroll the selected standalone worker and verify probe/compatibility")
        code = self.http("/api/enrollments", {}, expected=201)["code"]
        self.compose("run", "--rm", "-T", "--no-deps", "worker", "enroll", "--server", "http://server:8081",
                     "--name", "smoke", name="enroll", input=code + "\n")
        self.compose("up", "-d", "--no-build", "worker", name="start-worker")
        def online():
            row = self.worker_row()
            return row if row and row["status"] == "online" and row.get("last_seen_at") else None
        row = self.poll("first heartbeat", online)
        self.node = row["id"]
        saved = self.config()
        assert saved["node_id"] == self.node
        self.credential = saved["credential"]
        self.record["node_id"] = self.node
        response = self.http("/v1/worker/probe", {"nonce": "product-smoke"}, machine=True, credential=self.credential)
        assert response["nonce"] == "product-smoke"
        response = self.http("/v1/worker/heartbeat", {"worker_version": "fixture", "protocol_version": 99},
                             machine=True, credential=self.credential, expected=426)
        assert response["error"] == "unsupported_protocol" and response["supported_min"] == 1 and response["supported_max"] == 1
        execution_error = self.http("/v2/worker/register", {
            "identity": {"node_id": self.node, "boot_id": str(uuid.uuid4())},
            "operation_id": str(uuid.uuid4()), "protocol_version": 99,
            "worker_version": "fixture", "capabilities": []},
            machine=True, credential=self.credential, expected=426)
        assert execution_error["code"] == "unsupported_protocol"
        assert self.worker_row()["status"] == "online"
        self.say("restart daemon and PostgreSQL; preserve node identity and advance heartbeat")
        before = row["last_seen_at"]
        self.compose("restart", "server", "postgres", name="restart")
        self.address()
        self.poll("restarted health", lambda: self.http("/healthz"))
        def advanced():
            current = online()
            return current if current and current["id"] == self.node and current["last_seen_at"] > before else None
        after = self.poll("same-node reconnect", advanced)
        assert self.config() == saved
        self.record["foundation"] = {"first_heartbeat": before, "later_heartbeat": after["last_seen_at"],
                                     "identity_preserved": True, "probe": "passed", "unsupported_protocol": "passed"}
        self.record["foundation"]["unsupported_execution_preserves_connectivity"] = "passed"

    def execute(self):
        self.say("run synthetic assignment with lost acceptance and report acknowledgements")
        self.runtime = self.project + "-execution"
        self.compose("run", "-d", "--no-deps", "--name", self.runtime, "-e", "AQ_FIXTURE_HOLD=1", "worker",
                     "run-execution", "--state-dir", "/state/runtime", "--fixture-agent", "/usr/local/bin/aq-fixture-agent",
                     "--idle-timeout-secs", "90", "--exit-after-result", "--drop-acceptance-response-once",
                     "--drop-report-response-once", name="start-execution")
        def registration():
            output = self.run(["docker", "logs", self.runtime], name="execution-log")
            for line in output.splitlines():
                try:
                    event = json.loads(line)
                    if event.get("event") == "registered":
                        return event["identity"]
                except ValueError:
                    pass
        identity = self.poll("execution registration", registration)
        assert identity["node_id"] == self.node
        attempt = {"attempt_id": str(uuid.uuid4()), "session_id": str(uuid.uuid4()), "generation": 1}
        spec = {"project_id": str(uuid.uuid4()), "harness": "deterministic-agent-v1", "fixture_output": "product synthetic success",
                "resources": {"slots": 1, "cpu_millis": 100, "memory_bytes": 1048576, "disk_bytes": 1048576},
                "required_capabilities": ["deterministic-agent-v1"]}
        spec["resolved_spec_digest"] = sha(canonical(spec))
        offer = {"offer_id": str(uuid.uuid4()), "identity": identity, "attempt": attempt, "spec": spec,
                 "offer_expires_at_ms": int(time.time() * 1000) + 90000, "reservation_ttl_ms": 90000}
        # Pause only this disposable runtime so cancellation definitely precedes
        # its first opportunity to fetch the cancelled offer.
        self.run(["docker", "pause", self.runtime], name="pause-before-offers")
        cancelled = dict(offer, offer_id=str(uuid.uuid4()), attempt={
            "attempt_id": str(uuid.uuid4()), "session_id": str(uuid.uuid4()), "generation": 1})
        try:
            self.http("/api/execution/fixtures", {"task_id": str(uuid.uuid4()), "offer": cancelled})
            cancelled_state = self.http("/api/execution/attempts/" + cancelled["attempt"]["attempt_id"] + "/cancel",
                                        {"operation_id": str(uuid.uuid4()), "reason": "cancel before worker sees offer"})
            assert cancelled_state["status"] == "declined"
            self.http("/api/execution/fixtures", {"task_id": str(uuid.uuid4()), "offer": offer})
        finally:
            self.run(["docker", "unpause", self.runtime], name="unpause-after-offers", check=False)
        def claim():
            value = self.compose("run", "--rm", "-T", "--no-deps", "--entrypoint", "sh", "worker", "-ec",
                                 "find /state/runtime/launches -name started.claim -print", name="launch-claim", check=False)
            rows = [line for line in value.splitlines() if re.fullmatch(r"/state/runtime/launches/[0-9a-f-]{36}/started.claim", line)]
            if rows:
                assert len(rows) == 1, "duplicate synthetic process launch"
                return rows[0]
        started = self.poll("one durable process launch", claim)
        launch_id = started.split("/")[-2]
        self.run(["docker", "exec", self.runtime, "touch", started.rsplit("/", 1)[0] + "/release"], name="release-fixture")
        def exited():
            state = json.loads(self.run(["docker", "inspect", "--format", "{{json .State}}", self.runtime], name="execution-state"))
            if state["Status"] == "exited":
                assert state["ExitCode"] == 0, "standalone execution worker failed"
                return state
        self.poll("execution report acknowledgement", exited)
        self.run(["docker", "logs", self.runtime], name="execution-log")
        state = self.http("/api/execution/attempts/" + attempt["attempt_id"])
        assert state["status"] == "finished" and state["stopped"]
        assert state["disposition"]["disposition"] == "accepted"
        assert state["report"]["output"] == spec["fixture_output"]
        text = self.compose("run", "--rm", "-T", "--no-deps", "--entrypoint", "cat", "worker",
                            "/state/runtime/journal.json", name="execution-journal")
        journal = json.loads(text[text.index("{"):])
        assert len(journal["attempts"]) == 1
        local = journal["attempts"][attempt["attempt_id"]]
        assert local["launch_id"] == launch_id and local["phase"] == "retired"
        assert local["result"]["report_id"] == state["disposition"]["report_id"]
        replay = self.http("/v2/worker/result", state["report"], machine=True, credential=self.credential)
        assert replay == state["disposition"]
        self.compose("restart", "server", name="restart-after-result")
        self.address()
        recovered = self.poll("durable accepted result", lambda: self.http("/api/execution/attempts/" + attempt["attempt_id"]))
        assert recovered == state
        self.record["execution"] = {"attempt_id": attempt["attempt_id"], "boot_id": identity["boot_id"],
                                    "launch_id": launch_id, "launch_count": 1, "result": state,
                                    "acceptance_response_loss": "passed", "report_response_loss": "passed",
                                    "report_replay": "passed", "daemon_restart": "passed", "graph_completion": "not exercised"}
        self.record["execution"]["cancelled_unaccepted_attempt_id"] = cancelled["attempt"]["attempt_id"]
        self.record["execution"]["cancel_before_poll_then_acquire"] = "passed"

    def revoke(self):
        self.say("revoke credential; verify machine rejection and connector exit")
        self.http("/api/workers/" + self.node + "/revoke", {}, expected=204)
        response = self.http("/v1/worker/probe", {"nonce": "revoked"}, machine=True, credential=self.credential, expected=401)
        assert response["error"] == "credential_revoked"
        container = self.compose("ps", "-aq", "worker", name="worker-container").strip()
        def stopped():
            state = json.loads(self.run(["docker", "inspect", "--format", "{{json .State}}", container], name="revoked-worker-state"))
            return state if state["Status"] == "exited" else None
        state = self.poll("revoked worker exit", stopped)
        assert state["ExitCode"] == 3
        self.record["foundation"]["revocation"] = "passed (HTTP 401 and connector exit 3)"

    def cleanup(self):
        errors = []
        confirmed = True

        def attempt(action):
            try:
                return action()
            except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                errors.append(str(error))

        try:
            if self.dc:
                attempt(lambda: self.compose("logs", "--no-color", "--tail", "100", name="stack-logs", check=False))
                attempt(lambda: self.compose("down", "-v", "--remove-orphans", name="teardown", check=False))
            if self.runtime:
                attempt(lambda: self.run(["docker", "rm", "-fv", self.runtime], name="runtime-teardown", check=False))
            # Each operation proceeds even when Compose or log collection fails.
            # Selection is restricted to this fixture's random project label.
            for kind, listing, removal in [
                ("containers", ["docker", "ps", "-aq"], ["docker", "rm", "-fv"]),
                ("volumes", ["docker", "volume", "ls", "-q"], ["docker", "volume", "rm"]),
                ("networks", ["docker", "network", "ls", "-q"], ["docker", "network", "rm"]),
            ]:
                command = [*listing, "--filter", "label=com.docker.compose.project=" + self.project]
                try:
                    ids = self.run(command, name="remaining-" + kind).split()
                    if ids:
                        self.run([*removal, *ids], name="fallback-remove-" + kind)
                    if self.run(command, name="verified-" + kind).strip():
                        confirmed = False
                except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                    errors.append(str(error))
                    confirmed = False
            if self.images:
                attempt(lambda: self.run(["docker", "image", "rm", *self.images], name="image-teardown", check=False))
        finally:
            try:
                self.directory.cleanup()
            except OSError as error:
                errors.append(str(error))
            self.record["cleanup"] = {"owned_resources_removed": confirmed, "errors": errors}
            if not confirmed and self.record.get("outcome") == "passed":
                self.record["outcome"] = "failed"
                self.record["error"] = "owned resource cleanup could not be confirmed"
            self.record["finished_at"] = utc()
            (self.logs / "evidence.json").write_text(json.dumps(self.record, indent=2) + "\n")
            self.say("evidence " + str(self.logs))
        return confirmed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execution", action="store_true")
    args = parser.parse_args()
    fixture = Fixture(args.execution)
    status = 1
    try:
        fixture.setup()
        fixture.foundation()
        if args.execution:
            fixture.execute()
        fixture.revoke()
        fixture.record["outcome"] = "passed"
        status = 0
    except (OSError, ValueError, AssertionError, RuntimeError, subprocess.SubprocessError) as error:
        fixture.record["outcome"] = "failed"
        fixture.record["error"] = str(error)
        print(f"smoke: FAIL: {error}", file=sys.stderr)
    finally:
        if not fixture.cleanup():
            status = 1
    if status == 0:
        fixture.say("PASS")
    return status


if __name__ == "__main__":
    sys.exit(main())
