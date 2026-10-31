#!/usr/bin/env python3
"""Verify the exact normalized public lifecycle dependency without sibling checkout."""
import hashlib,json,pathlib,tarfile,tomllib
root=pathlib.Path(__file__).resolve().parents[1]
vendor=root/'vendor'
m=json.loads((vendor/'lifecycle-contract.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
archive=vendor/m['archive']
assert sha(archive.read_bytes())==m['archive_sha256']
prefix=f"{m['package']}-{m['version']}/"
with tarfile.open(archive) as tar:
    entries={x.name[len(prefix):]:tar.extractfile(x).read() for x in tar.getmembers() if x.isfile()}
assert set(entries)=={x['path'] for x in m['files']}
for f in m['files']:
    b=entries[f['path']]
    assert len(b)==f['bytes'] and sha(b)==f['sha256'],f['path']
    assert (vendor/prefix/f['path']).read_bytes()==b,f['path']
vcs=json.loads(entries['.cargo_vcs_info.json'])
assert vcs['git']['sha1']==m['source_revision'] and not vcs['git'].get('dirty',False)
cargo=tomllib.loads(entries['Cargo.toml'].decode())
assert cargo['dependencies']['aq-session-protocol']['version']=='=0.1.0'
assert sha((vendor/'aq-session-protocol-0.1.0.crate').read_bytes())==m['session_dependency']['archive_sha256']
consumer=tomllib.loads((root/'crates/aq-lifecycle-api/Cargo.toml').read_text())
assert consumer['dependencies']['aq-lifecycle-protocol']['version']=='=0.1.0'
print(f"verified {m['package']} {m['version']} at {m['source_revision']}")
public_fixtures=json.loads(entries['artifacts/fixtures.json'])
attachment=next(p['body']['message']['body'] for p in public_fixtures['valid_packets'] if p['body']['message']['kind']=='continuation')
assert json.loads((root/'crates/aq-lifecycle-api/artifacts/public-continuation.json').read_text())==attachment
