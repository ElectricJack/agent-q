#!/usr/bin/env python3
"""Verify the pinned session contract package without a sibling checkout."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def check(root=ROOT):
    vendor = root / "vendor"
    manifest = json.loads((vendor / "session-contract.json").read_text())
    revision = manifest["source_revision"]
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("session contract source revision is not immutable")
    name, version = manifest["package"], manifest["version"]
    if name != "aq-session-protocol" or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("unexpected session contract package")
    prefix = f"{name}-{version}"
    if manifest["archive"] != prefix + ".crate":
        raise ValueError("unexpected session contract archive path")
    archive = vendor / manifest["archive"]
    if sha256(archive.read_bytes()) != manifest["archive_sha256"]:
        raise ValueError("session contract archive digest mismatch")
    expected = {item["path"]: item["sha256"] for item in manifest["files"]}
    if len(expected) != len(manifest["files"]) or not expected:
        raise ValueError("duplicate or empty session contract inventory")
    packaged = {}
    with tarfile.open(archive) as tar:
        for member in tar:
            path = PurePosixPath(member.name)
            if (not member.isfile() or path.is_absolute() or ".." in path.parts
                    or path.parts[0] != prefix or len(path.parts) < 2):
                raise ValueError("unsafe session contract archive member")
            relative = str(PurePosixPath(*path.parts[1:]))
            if relative in packaged:
                raise ValueError("duplicate session contract archive member")
            packaged[relative] = tar.extractfile(member).read()
    if set(packaged) != set(expected):
        raise ValueError("session contract archive inventory disagrees")
    local = vendor / prefix
    local_paths = set()
    for path in local.rglob("*"):
        if path.is_symlink():
            raise ValueError("session contract package contains a symlink")
        if path.is_file():
            local_paths.add(path.relative_to(local).as_posix())
    if local_paths != set(expected):
        raise ValueError("vendored session contract inventory disagrees")
    for relative, content in packaged.items():
        if sha256(content) != expected[relative] or (local / relative).read_bytes() != content:
            raise ValueError(f"session contract content drift: {relative}")
    vcs = json.loads(packaged[".cargo_vcs_info.json"])
    if vcs["git"]["sha1"] != revision or vcs["git"].get("dirty", False):
        raise ValueError("session contract was not packaged from the selected clean revision")
    package = tomllib.loads(packaged["Cargo.toml"].decode())["package"]
    if package["name"] != name or package["version"] != version:
        raise ValueError("session contract package metadata disagrees")
    return manifest


if __name__ == "__main__":
    try:
        manifest = check()
    except (KeyError, OSError, ValueError, tarfile.TarError) as error:
        raise SystemExit(f"session contract: {error}")
    print(f"PASS: {manifest['package']} {manifest['version']} at {manifest['source_revision']}")
