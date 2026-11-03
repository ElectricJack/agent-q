"""Bounded Cargo execution through AQ's pytest resource gate."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
COMMANDS = {
    "migration-format": ["cargo", "test", "--locked", "-p", "aq-server", "--lib", "state_format_is_the_latest_migration"],
    "migration-merge": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permits", "--test", "delegated_permit_runtime", "--test", "central_sessions"],
    "legacy-renewal": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "native_initial_grant", "original_cold_inventory_actual_installation_can_renew_without_retiming_originals"],
    "renewal": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permit_runtime", "--test", "phase_refresh_terminal", "--test", "phase_terminal_started"],
    "polls": ["cargo", "test", "--locked", "-p", "aq-server", "--lib", "delegated"],
    "runtime": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permit_runtime", "--test", "permit_fixture_conformance"],
    "check": ["cargo", "check", "--locked", "-p", "aq-server"],
    "conformance": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "permit_fixture_conformance"],
    "model": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permits"],
    "area": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "native_cancellation"],
    "final": ["cargo", "test", "--locked", "-p", "aq-server", "--test", "delegated_permits", "--test", "native_cancellation", "--", "--test-threads=1"],
    "format": ["cargo", "fmt", "-p", "aq-server", "--check"],
    "clippy": ["cargo", "clippy", "--locked", "-p", "aq-server", "--lib", "--test", "delegated_permits", "--test", "delegated_permit_runtime", "--test", "permit_fixture_conformance", "--", "-D", "warnings"],
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
