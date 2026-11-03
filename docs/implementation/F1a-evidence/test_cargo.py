"""Focused Rust checks through AQ's pytest resource gate."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
COMMANDS = {
    "owner": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "central_sessions", "--", "--test-threads=1"],
    "model": ["cargo", "test", "--locked", "-p", "aq-central-authority"],
    "format": ["cargo", "fmt", "--check"],
    "clippy": ["cargo", "clippy", "--locked", "-p", "aq-central-authority", "-p", "aq-server", "--lib", "--test", "central_sessions", "--", "-D", "warnings"],
    "format_version": ["cargo", "test", "--locked", "-p", "aq-server", "--lib", "state_format_is_the_latest_migration"],
}


def test_cargo():
    check = os.environ.get("F1A_CHECK", "owner")
    environment = os.environ.copy()
    environment.setdefault("CARGO_BUILD_JOBS", "2")
    environment.setdefault("CARGO_PROFILE_DEV_DEBUG", "0")
    environment.setdefault("CARGO_INCREMENTAL", "0")
    environment.setdefault("RUST_TEST_THREADS", "2")
    log = Path(__file__).with_name(f"{check}.txt")
    with log.open("w") as output:
        result = subprocess.run(COMMANDS[check], cwd=ROOT, env=environment,
                                stdout=output, stderr=subprocess.STDOUT)
    assert result.returncode == 0, f"{COMMANDS[check]} exited {result.returncode}; see {log}"
