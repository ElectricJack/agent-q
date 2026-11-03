"""E5 host operations: disk pruning, off-host backups and the live VPS checks.

The shell scripts run against fake `docker`, `df` and `restic` binaries that
log their argv; the checks run against local sockets and a local HTTP server.
"""
import http.server
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import threading

import pytest

ROOT = Path(__file__).resolve().parents[1]
PRUNE = ROOT / "deploy/aq-prune.sh"
BACKUP = ROOT / "deploy/aq-backup.sh"
CHECKS = ROOT / "scripts/vps-checks.py"

FAKE = r"""#!/bin/sh
printf '%s\n' "$(basename "$0") $*" >> "$FAKE_LOG"
case "$(basename "$0") $*" in
  "df "*) printf 'Use%%\n %s%%\n' "$FAKE_USE" ;;
  *"pg_dump"*"$FAKE_FAIL_DB"*) [ -n "$FAKE_FAIL_DB" ] && exit 1; printf 'PGDMP' ;;
  *"pg_dump"*) printf 'PGDMP' ;;
  *"volume ls"*) printf 'cargo-cache\n' ;;
esac
exit 0
"""


@pytest.fixture
def fakes(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for name in ("docker", "df", "restic"):
        path = bin_dir / name
        path.write_text(FAKE)
        path.chmod(0o755)
    log = tmp_path / "log"
    log.touch()
    config = tmp_path / "etc-agent-q"
    config.mkdir()
    (config / "agent-q.env").write_text("AQ_VAULT_DIR=/etc/agent-q/vault\n")

    def run(script, use=10, fail_db="", **extra):
        env = dict(os.environ, PATH=f"{bin_dir}:{os.environ['PATH']}", FAKE_LOG=str(log),
                   FAKE_USE=str(use), FAKE_FAIL_DB=fail_db,
                   AQ_SANDBOX_SOCKET="/run/user/1001/aq-sandbox/podman.sock",
                   AQ_CONFIG_DIR=str(config), AQ_BACKUP_STAGING=str(tmp_path / "staging"),
                   RESTIC_REPOSITORY="s3:example.invalid/aq-backups", **extra)
        result = subprocess.run(["sh", str(script)], env=env, capture_output=True, text=True)
        return result, log.read_text().splitlines()

    return run


def sandbox(lines):
    return [l for l in lines if l.startswith("docker -H unix:///run/user/1001/aq-sandbox/podman.sock")]


def engine(lines):
    return [l for l in lines if l.startswith("docker ") and " -H " not in l]


def test_prune_light_below_threshold(fakes):
    result, lines = fakes(PRUNE, use=40)
    assert result.returncode == 0, result.stderr
    sb = " | ".join(sandbox(lines))
    assert "container prune -f" in sb and "image prune -f" in sb
    assert "image prune -a" not in sb and "volume rm" not in sb
    assert any("image prune -f" in l for l in engine(lines))


def test_prune_deep_above_threshold(fakes):
    result, lines = fakes(PRUNE, use=71)
    assert result.returncode == 0, result.stderr
    sb = " | ".join(sandbox(lines))
    assert "image prune -a -f" in sb
    assert "volume ls -q -f dangling=true" in sb
    assert "volume rm cargo-cache" in sb


def test_prune_never_prunes_tagged_engine_images(fakes):
    for use in (10, 95):
        result, lines = fakes(PRUNE, use=use)
        assert result.returncode == 0, result.stderr
        for line in engine(lines):
            assert " -a" not in line and "--all" not in line and "rmi" not in line
            assert "volume" not in line and "system prune" not in line


def test_backup_dumps_both_databases_and_config(fakes, tmp_path):
    result, lines = fakes(BACKUP)
    assert result.returncode == 0, result.stderr
    dumps = [l for l in lines if "pg_dump" in l]
    assert any(l.endswith("-Fc aq_server") for l in dumps)
    assert any(l.endswith("-Fc aq_graph") for l in dumps)
    assert all("exec -T postgres pg_dump -U agent_q" in l for l in dumps)
    backup = next(l for l in lines if l.startswith("restic backup"))
    assert str(tmp_path / "staging") in backup and str(tmp_path / "etc-agent-q") in backup
    assert not list((tmp_path / "staging").glob("*.dump")), "plaintext dumps must not stay on disk"


def test_backup_fails_when_dump_fails(fakes):
    result, lines = fakes(BACKUP, fail_db="aq_graph")
    assert result.returncode != 0
    assert not any(l.startswith("restic backup") for l in lines)


def test_backup_applies_retention(fakes):
    result, lines = fakes(BACKUP)
    assert result.returncode == 0, result.stderr
    forget = next(l for l in lines if l.startswith("restic forget"))
    for flag in ("--keep-daily 7", "--keep-weekly 4", "--keep-monthly 6", "--prune"):
        assert flag in forget


class Machine(http.server.BaseHTTPRequestHandler):
    """The worker listener behind a Service Auth Access app."""
    open_human_routes = False

    def do_GET(self):
        if self.headers.get("CF-Access-Client-Id") != "id.access":
            self.send_response(403)
        elif self.path in ("/healthz", "/readyz"):
            self.send_response(200)
        elif self.open_human_routes and self.path.startswith("/api/"):
            self.send_response(200)
        else:
            self.send_response(404)
        self.end_headers()

    def log_message(self, *args):
        pass


def serve(handler):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def checks(*args, **env):
    full = dict(os.environ, CF_ACCESS_CLIENT_ID="id.access", CF_ACCESS_CLIENT_SECRET="secret", **env)
    result = subprocess.run([sys.executable, str(CHECKS), "--timeout", "1", *args],
                            env=full, capture_output=True, text=True)
    return result, json.loads(result.stdout or "{}")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def test_checks_origin_ports_closed():
    port = free_port()
    result, report = checks("origin", "--host", "127.0.0.1", "--ports", str(port))
    assert result.returncode == 0, result.stderr
    assert report["origin"]["open"] == [] and report["ok"]


def test_checks_flag_open_origin():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        port = listener.getsockname()[1]
        result, report = checks("origin", "--host", "127.0.0.1", "--ports", f"{port},{free_port()}")
    assert result.returncode == 1
    assert report["origin"]["open"] == [port] and not report["ok"]


@pytest.mark.parametrize("open_human", [False, True])
def test_checks_machine_host_refuses_human_routes(open_human):
    handler = type("H", (Machine,), {"open_human_routes": open_human})
    server = serve(handler)
    try:
        url = f"http://127.0.0.1:{server.server_port}"
        result, report = checks("machine", "--url", url)
    finally:
        server.shutdown()
    machine = report["machine"]
    assert machine["without_token"]["/healthz"] == 403
    assert machine["with_token"]["/healthz"] == 200
    if open_human:
        assert result.returncode == 1 and not report["ok"]
        assert machine["with_token"]["/api/me"] == 200
    else:
        assert result.returncode == 0, result.stderr
        assert all(machine["with_token"][r] == 404 for r in machine["human_routes"])
