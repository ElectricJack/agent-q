"""Dev bootstrap regression tests: exercise the command and its persisted state."""
import json
import os
from pathlib import Path
import subprocess

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
