"""Dev bootstrap regression tests: exercise the command and its persisted state."""
import json
import os
from pathlib import Path
import subprocess
import importlib.util
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

ROOT = Path(__file__).resolve().parents[1]


def command(tmp_path, *args, extra=None):
    env = dict(os.environ, AQ_DEV_STATE_DIR=str(tmp_path / "state"),
               AQ_SECRETS_DIR=str(tmp_path / "secrets"))
    for key in ("AQ_VAULT_DIR", "DEV_SCRATCH_REPO_URL", "DEV_SCRATCH_BASE_BRANCH",
                "DEV_SCRATCH_CREDENTIAL_REF"):
        env.pop(key, None)
    env.update(extra or {})
    return subprocess.run(["python3", str(ROOT / "scripts/dev-stack.py"), *args],
                          env=env, text=True, capture_output=True)


def test_prepare_replay_preserves_private_credentials_and_config(tmp_path):
    first = command(tmp_path, "prepare")
    assert first.returncode == 0, first.stderr
    env = json.loads(first.stdout)
    token = tmp_path / "secrets/graph_token"
    original = token.read_bytes()
    assert len(original.strip()) >= 32
    assert token.stat().st_mode & 0o777 == 0o600
    assert original not in first.stdout.encode()
    assert command(tmp_path, "prepare").returncode == 0
    assert token.read_bytes() == original
    vault = Path(env["AQ_VAULT_DIR"]) / "config.toml"
    import tomllib
    config = tomllib.loads(vault.read_text())
    assert config["auth"]["mode"] == "dev"
    assert config["agent_access"]["graph_url"] == "http://aq-graph:8741"
    assert config["source_retention"]["git_socket"] == "/git-run/aq-git.sock"
    assert command(tmp_path, "prepare").stdout == first.stdout


def test_seed_unset_or_partial_designation_fails_before_contacting_services(tmp_path):
    for extra in ({}, {"DEV_SCRATCH_REPO_URL": "https://github.com/example/scratch.git"}):
        result = command(tmp_path, "seed", extra=extra)
        assert result.returncode != 0
        assert "DEV_SCRATCH" in result.stderr
        assert "Traceback" not in result.stderr
        assert not (tmp_path / "state/seed.json").exists()


def test_seed_rejects_credentials_in_url_and_unsupported_credential_profiles(tmp_path):
    for url, credential in (("https://user:secret@github.com/owner/scratch.git", "none"),
                            ("https://github.com/owner/scratch.git?token=secret", "none"),
                            ("https://github.com/owner/scratch.git", "private-profile")):
        result = command(tmp_path, "seed", extra=dict(DEV_SCRATCH_REPO_URL=url,
            DEV_SCRATCH_BASE_BRANCH="main", DEV_SCRATCH_CREDENTIAL_REF=credential))
        assert result.returncode != 0
        assert "DEV_SCRATCH" in result.stderr
        assert "secret" not in result.stderr
        assert not (tmp_path / "state/seed.json").exists()


def test_seed_refuses_retained_credentials_after_database_reset_before_enrolling(tmp_path):
    assert command(tmp_path, "prepare").returncode == 0
    credential = tmp_path / "secrets/dev_worker.json"
    original = b'{"node_id":"old-node","credential":"aqn_retained-secret"}'
    credential.write_bytes(original)
    posts = []

    class EmptyWorkerDatabase(BaseHTTPRequestHandler):
        def do_GET(self):
            assert self.path == "/api/workers"
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"[]")

        def do_POST(self):
            posts.append(self.path)
            self.send_response(500)
            self.end_headers()

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), EmptyWorkerDatabase)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        result = command(tmp_path, "seed", extra=dict(
            DEV_SCRATCH_REPO_URL="https://github.com/example/scratch.git", DEV_SCRATCH_BASE_BRANCH="main",
            DEV_SCRATCH_CREDENTIAL_REF="none", AQ_DEV_SERVER_URL=f"http://127.0.0.1:{server.server_port}"))
        assert result.returncode != 0
        assert "worker credential has no matching worker" in result.stderr
        assert not posts, "seed enrolled another node before refusing the stale credential"
        assert credential.read_bytes() == original
        assert "retained-secret" not in result.stderr
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def load_dev():
    spec = importlib.util.spec_from_file_location("e0_dev_stack", ROOT / "scripts/dev-stack.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_interrupted_credential_write_never_publishes_a_partial_record(tmp_path, monkeypatch):
    dev = load_dev()
    credential = tmp_path / "dev_worker.json"

    def interrupted_write(record, output):
        output.write('{"credential":')
        raise OSError("simulated disk failure")

    monkeypatch.setattr(dev.json, "dump", interrupted_write)
    with pytest.raises(OSError, match="disk failure"):
        dev.save_worker_credential(credential, {})
    assert not credential.exists()
    assert list(tmp_path.iterdir()) == []


def test_atomic_credential_publish_preserves_an_existing_record(tmp_path):
    dev = load_dev()
    credential = tmp_path / "dev_worker.json"
    dev.save_worker_credential(credential, {"credential": "first"})
    assert credential.stat().st_mode & 0o777 == 0o600
    with pytest.raises(FileExistsError):
        dev.save_worker_credential(credential, {"credential": "replacement"})
    assert json.loads(credential.read_text()) == {"credential": "first"}
    assert list(tmp_path.iterdir()) == [credential]
