"""Bounded Cargo execution through AQ's pytest resource gate."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
COMMANDS = {
    "model": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permits"],
    "area": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "native_cancellation"],
    "final": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permits", "--test", "native_cancellation", "--", "--test-threads=1"],
    "format": ["cargo", "fmt", "-p", "aq-server", "--check"],
    "clippy": ["cargo", "clippy", "--locked", "-p", "aq-server", "--lib", "--test", "delegated_permits", "--", "-D", "warnings"],
}


def test_cargo():
    check = os.environ.get("B2_CHECK", "model")
    environment = os.environ.copy()
    environment.setdefault("CARGO_BUILD_JOBS", "2")
    environment.setdefault("CARGO_PROFILE_DEV_DEBUG", "0")
    environment.setdefault("CARGO_INCREMENTAL", "0")
    environment.setdefault("RUST_TEST_THREADS", "2")
    log = Path(__file__).with_name(os.environ.get("B2_LOG", check) + ".txt")
    with log.open("w") as output:
        result = subprocess.run(COMMANDS[check], cwd=ROOT, env=environment,
                                stdout=output, stderr=subprocess.STDOUT)
    assert result.returncode == 0, f"{COMMANDS[check]} exited {result.returncode}; see {log}"
