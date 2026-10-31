#!/usr/bin/env python3
"""Verify the immutable standalone worker source and public contract inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def candidate(official=False):
    if official and any(os.environ.get(k) for k in ("AQ_WORKER_BIN", "AQ_WORKER_SOURCE_DIR")):
        raise ValueError("official builds reject local worker overrides")
    manifest = json.loads((ROOT / "vendor/worker-candidate.json").read_text())
    if not re.fullmatch(r"[0-9a-f]{40}", manifest["source_revision"]):
        raise ValueError("worker source must name a full immutable commit")
    archive = ROOT / "vendor" / manifest["archive"]
    if archive.parent != ROOT / "vendor" or digest(archive.read_bytes()) != manifest["archive_sha256"]:
        raise ValueError("worker source archive digest mismatch")
    expected = f'{manifest["archive_sha256"]}  {manifest["archive"]}\n'
    if (ROOT / "vendor/worker-candidate.sha256").read_text() != expected:
        raise ValueError("worker archive checksum file disagrees with its manifest")
    contracts = json.loads((ROOT / "vendor/worker-contracts.json").read_text())
    with tarfile.open(archive) as tar:
        workspace = tomllib.loads(tar.extractfile("Cargo.toml").read().decode())["workspace"]
        if tar.pax_headers.get("comment") != manifest["source_revision"]:
            raise ValueError("source archive commit identity disagrees with candidate")
        worker_info = tomllib.loads(tar.extractfile("crates/aq-worker/Cargo.toml").read().decode())["package"]
        worker_version = worker_info["version"]
        if isinstance(worker_version, dict) and worker_version.get("workspace"):
            worker_version = workspace["package"]["version"]
        if worker_version != manifest["version"]:
            raise ValueError("worker package version disagrees with candidate")
        for name in tar.getnames():
            if Path(name).is_absolute() or ".." in Path(name).parts:
                raise ValueError("unsafe source archive member")
        for member in tar.getmembers():
            if member.issym() or member.islnk():
                if Path(member.linkname).is_absolute() or ".." in Path(member.linkname).parts:
                    raise ValueError("unsafe source archive link")
            if not (member.isfile() or member.isdir() or member.issym() or member.islnk()):
                raise ValueError("unsupported source archive member")
        for package in contracts["packages"]:
            base = ROOT / "vendor" / f'{package["name"]}-{package["version"]}'
            for item in package["files"]:
                if digest((base / item["path"]).read_bytes()) != item["sha256"]:
                    raise ValueError(f'public contract changed: {package["name"]}/{item["path"]}')
            prefix = f'crates/{package["name"]}/'
            # Cargo generates these during packaging. Cargo.toml.orig is the
            # original manifest and must match the archive's Cargo.toml.
            for item in package["files"]:
                path = item["path"]
                if path in {".cargo_vcs_info.json", "Cargo.lock", "Cargo.toml"}:
                    continue
                source_path = "Cargo.toml" if path == "Cargo.toml.orig" else path
                source = tar.extractfile(prefix + source_path).read()
                if source != (base / path).read_bytes():
                    raise ValueError(f'worker and daemon public package disagree: {package["name"]}/{source_path}')
            expected_sources = {item["path"] for item in package["files"] if item["path"].startswith("src/")}
            actual_sources = {member.name[len(prefix):] for member in tar.getmembers()
                              if member.isfile() and member.name.startswith(prefix + "src/")}
            if actual_sources != expected_sources:
                raise ValueError(f'worker and daemon contract source sets disagree: {package["name"]}')
            for source_path in expected_sources:
                if tar.extractfile(prefix + source_path).read() != (base / source_path).read_bytes():
                    raise ValueError(f'worker and daemon contract disagree: {package["name"]}/{source_path}')
            package_info = tomllib.loads(tar.extractfile(prefix + "Cargo.toml").read().decode())["package"]
            version = package_info["version"]
            if isinstance(version, dict) and version.get("workspace"):
                version = workspace["package"]["version"]
            if version != package["version"]:
                raise ValueError(f'worker and daemon contract versions disagree: {package["name"]}')
        lock = tar.extractfile("Cargo.lock").read()
        if digest(lock) != manifest["lockfile_sha256"]:
            raise ValueError("worker lockfile identity disagrees with candidate")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--official", action="store_true")
    parser.add_argument("--revision", action="store_true")
    args = parser.parse_args()
    try:
        value = candidate(args.official)
    except (ValueError, KeyError, OSError, tarfile.TarError) as error:
        parser.exit(1, f"worker candidate: {error}\n")
    print(value["source_revision"] if args.revision else json.dumps(value, indent=2))
