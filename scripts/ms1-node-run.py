#!/usr/bin/env python3
"""MS1: one graph attempt on the desktop node through the aq-ms1 stack.

Host port of aq-worker scripts/e0-qual/i0-run-native.py. It preserves any
previous node state, starts a worker-side aq-git on a fresh state root,
registers the scratch repository's worker-mode binding, then runs
`aq-worker run-native` once with the Claude docker-local executor
(native.json from scripts/ms1-profiled.py). Prints public status only; the
Claude token never passes through this script.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
import uuid

BASE = Path(os.environ.get("AQ_MS1_HOME", Path.home() / ".local/share/aq-ms1"))
NODE = BASE / "node"
STATE = NODE / "state"
RUNS = NODE / "runs"
RUNTIME = BASE / "runtime"
REPOSITORY = "e0000000-0000-4000-8000-000000000003"
IDENTITY = "e0-development"  # must equal the central binding identity
GENERATION = 1


def exchange(request):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(10)
        client.connect(str(STATE / "git.sock"))
        client.sendall(json.dumps(request, separators=(",", ":")).encode() + b"\n")
        with client.makefile("rb") as stream:
            frame = stream.readline(65537)
    reply = json.loads(frame)
    if reply.get("ok") is not True or reply.get("request_id") != request["request_id"]:
        raise RuntimeError(f"aq-git refused {request['operation']}: {json.dumps(reply.get('error'))[:500]}")
    return reply["result"]


def stop(child):
    if child is not None and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)


def fresh_state(stamp):
    RUNS.mkdir(mode=0o700, exist_ok=True)
    if STATE.exists() and any(STATE.iterdir()):
        if (STATE / "journal").exists():
            shutil.move(str(STATE), str(RUNS / f"state-{stamp}"))
        else:
            shutil.rmtree(STATE)
    STATE.mkdir(mode=0o700, exist_ok=True)
    os.chmod(STATE, 0o700)
    for d in ("host", "git"):
        (STATE / d).mkdir(mode=0o700)


def main():
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    fresh_state(stamp)
    git = worker = None
    log = RUNS / f"run-{stamp}.log"
    try:
        git = subprocess.Popen([str(RUNTIME / "aq-git"), "--state-dir", str(STATE / "git"),
                                "--listen", str(STATE / "git.sock"), "--git-executable", "/usr/bin/git", "serve"],
                               stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               close_fds=True, start_new_session=True)
        deadline = time.monotonic() + 10
        while not (STATE / "git.sock").exists():
            if git.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("worker-side aq-git did not listen")
            time.sleep(0.05)
        operation = str(uuid.uuid4())
        registered = exchange({
            "protocol_version": 1, "request_id": operation, "operation_id": operation,
            "operation": "repo.register", "repository_id": REPOSITORY, "expected_binding_generation": 0,
            "params": {"binding": {"identity": IDENTITY, "mode": "worker",
                                   "binding_generation": GENERATION, "object_format": "sha1"}}})
        if registered.get("mode") != "worker":
            raise RuntimeError("worker binding mismatch")
        print(json.dumps({"event": "worker_binding", "repository_id": REPOSITORY, "mode": "worker",
                          "at": time.strftime("%H:%M:%SZ", time.gmtime())}), flush=True)
        started = time.time()
        env = dict(os.environ, RUST_LOG=os.environ.get("RUST_LOG", "info"))
        with open(log, "w") as out:
            worker = subprocess.Popen([str(RUNTIME / "aq-worker"), "--config", str(NODE / "config/worker.toml"),
                                       "run-native", "--native-config", str(NODE / "config/native.json")],
                                      stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT,
                                      close_fds=True, env=env)
            code = worker.wait()
        tail = log.read_text().splitlines()[-12:]
        print("\n".join(tail), flush=True)
        print(json.dumps({"event": "run_native_exit", "exit_code": code, "log": str(log),
                          "wall_seconds": round(time.time() - started, 1)}), flush=True)
        return code
    finally:
        stop(worker)
        stop(git)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # public status only
        print(f"ms1 node run refused: {error}", file=sys.stderr, flush=True)
        sys.exit(2)
