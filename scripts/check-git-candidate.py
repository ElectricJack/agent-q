#!/usr/bin/env python3
"""Verify retained immutable aq-git source; never consult a sibling checkout."""
import argparse,hashlib,json,os,re,tarfile,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def verify_build_stages(manifest):
    """Require application Dockerfiles to embed the exact canonical builders."""
    canonical = (ROOT / 'scripts/Dockerfile.aq-git').read_text()
    stages = list(re.finditer(r'^FROM ', canonical, re.MULTILINE))
    if len(stages) != 3:
        raise ValueError('canonical aq-git recipe must have two builders and one runtime')
    expected = canonical[:stages[2].start()]
    if expected.count('FROM ' + manifest['builder']) != 2:
        raise ValueError('canonical builder disagrees with candidate manifest')
    if manifest['archive_sha256'] not in expected or manifest['git']['sha256'] not in expected:
        raise ValueError('canonical source checksums disagree with candidate manifest')
    begin = '# BEGIN PINNED AQ-GIT BUILD STAGES\n'
    end = '# END PINNED AQ-GIT BUILD STAGES\n'
    for name in ('Dockerfile', 'Dockerfile.worker'):
        content = (ROOT / name).read_text()
        if content.count(begin) != 1 or content.count(end) != 1:
            raise ValueError(f'{name}: exactly one pinned aq-git block required')
        actual = content.split(begin, 1)[1].split(end, 1)[0]
        if actual != expected:
            raise ValueError(f'{name}: pinned aq-git builders drifted from scripts/Dockerfile.aq-git')

def candidate(official=False):
    if official and any(os.environ.get(k) for k in ('AQ_GIT_BIN','AQ_GIT_SOURCE_DIR')):
        raise ValueError('official builds reject local aq-git overrides')
    m=json.loads((ROOT/'vendor/git-candidate.json').read_text())
    if not re.fullmatch('[0-9a-f]{40}',m['source_revision']): raise ValueError('full revision required')
    archive=ROOT/'vendor'/m['archive']
    if archive.parent != ROOT/'vendor' or hashlib.sha256(archive.read_bytes()).hexdigest()!=m['archive_sha256']:
        raise ValueError('aq-git source digest mismatch')
    with tarfile.open(archive) as t:
        inventory=json.loads((ROOT/'vendor/aq-git-0.1.0/source-inventory.json').read_text())
        actual={x.name:hashlib.sha256(t.extractfile(x).read()).hexdigest() for x in t.getmembers() if x.isfile()}
        if actual!={x['path']:x['sha256'] for x in inventory}: raise ValueError('source inventory mismatch')
        for x in t.getmembers():
            if Path(x.name).is_absolute() or '..' in Path(x.name).parts or not (x.isfile() or x.isdir()):
                raise ValueError('unsafe source archive member')
        if not t.extractfile('Cargo.lock'): raise ValueError('locked build required')
    git=m['git']; source=ROOT/'vendor'/git['archive']
    if source.resolve().parent!= (ROOT/'vendor/aq-git-0.1.0').resolve() or hashlib.sha256(source.read_bytes()).hexdigest()!=git['sha256']: raise ValueError('Git runtime source digest mismatch')
    if hashlib.sha256((ROOT/'vendor/aq-git-source.bundle').read_bytes()).hexdigest()!=m['bundle_sha256']: raise ValueError('Git bundle digest mismatch')
    expected=f"{m['archive_sha256']}  {m['archive']}\n{m['bundle_sha256']}  aq-git-source.bundle\n{git['sha256']}  {git['archive']}\n"
    if (ROOT/'vendor/git-candidate.sha256').read_text()!=expected: raise ValueError('candidate checksum file mismatch')
    heads=subprocess.check_output(['git','bundle','list-heads',str(ROOT/'vendor/aq-git-source.bundle')],text=True).splitlines()
    if f"{m['source_revision']} HEAD" not in heads: raise ValueError('bundle commit identity mismatch')
    if official:
        verify_build_stages(m)
    return m
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--official',action='store_true');a=p.parse_args()
    print(json.dumps(candidate(a.official),sort_keys=True))
