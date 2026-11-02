#!/usr/bin/env python3
"""Explicit two-case local fixture qualification; never a canonical service probe."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--pins', type=Path, required=True)
parser.add_argument('--evidence', type=Path, required=True)
args = parser.parse_args()
assert os.environ.get('AQ_TEST_ALLOW_PHASE_GIT') == '1', 'explicit disposable opt-in required'
assert args.evidence.is_dir() and not args.evidence.is_symlink()
assert args.evidence.stat().st_mode & 0o777 == 0o700
pins = json.loads(args.pins.read_text())
assert pins['minimum_free_bytes'] == 11005853696, 'preserve approved default minimum'
for path, expected in [(pins['binary'], pins['binary_sha256']),
                       (pins['aq_git'], pins['aq_git_sha256']),
                       (pins['git'], pins['git_sha256'])]:
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            digest.update(block)
    assert digest.hexdigest() == expected, 'qualified executable changed'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == pins['source']
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
version = subprocess.check_output([pins['git'], '--version'], text=True).strip()
assert version == 'git version 2.43.0'
env = dict(os.environ, AQ_TEST_GIT_CANDIDATE=pins['aq_git'],
           AQ_TEST_GIT_CANDIDATE_SHA256=pins['aq_git_sha256'],
           AQ_TEST_GIT_EXECUTABLE=pins['git'],
           AQ_TEST_GIT_EXECUTABLE_SHA256=pins['git_sha256'])
summary = dict(pins=pins, actual_git_version=version, cases=[])
for family in ('frozen', 'refresh'):
    stat = os.statvfs('/tmp')
    free = stat.f_bavail * stat.f_frsize
    assert free >= pins['minimum_free_bytes'], 'default free-space floor refuses actual case'
    case = family + '::actual_phase_stopped_checkpoint_http_import_and_current_retention_are_not_legacy_gated'
    log = args.evidence / (family + '.log')
    started = time.monotonic()
    with log.open('xb') as output:
        log.chmod(0o600)
        process = subprocess.Popen([pins['binary'], case, '--exact', '--ignored',
                                    '--nocapture', '--test-threads=1'], stdout=output,
                                   stderr=subprocess.STDOUT, env=env, start_new_session=True)
        timed_out = False
        cleanup_unknown = False
        try:
            code = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
            # Only the still-owned Popen case leader's original group is signaled.
            # Git owners have their own groups. No name-based broad cleanup occurs.
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    cleanup_unknown = True
            code = process.returncode
    text = log.read_text()
    owners = [dict(group=int(g), start=int(t), session=int(s), root=r)
              for g, t, s, r in re.findall(
                  r'SOURCE_RETENTION_OWNER group=(\d+) start=(\d+) session=(\d+) root=(\S+)', text)]
    # The unique SID catches nested legitimate PGIDs, even after reparenting.
    # Private-root residual checks conservatively catch observable argv escapes.
    remaining = []
    unknown = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            tail = (entry / 'stat').read_text().rsplit(') ', 1)[1].split()
            session = int(tail[3])
            command = (entry / 'cmdline').read_bytes().replace(b'\0', b' ').decode(errors='replace')
        except (FileNotFoundError, ProcessLookupError):
            continue
        except (OSError, ValueError, IndexError) as error:
            unknown.append(dict(pid=entry.name, error=type(error).__name__))
            continue
        if session == process.pid or any(owner['root'] in command for owner in owners):
            remaining.append(dict(pid=entry.name, session=session, command=command))
    drained = {int(g) for g in re.findall(r'SOURCE_RETENTION_OWNER_DRAINED group=(\d+)', text)}
    item = dict(case=case, exit_code=code, elapsed_seconds=round(time.monotonic()-started, 3),
                available_before_case=free, minimum_free_bytes=pins['minimum_free_bytes'],
                timeout=timed_out, cleanup_unknown=cleanup_unknown,
                owners=owners, drained_groups=sorted(drained),
                remaining=remaining, unknown_observations=unknown,
                state_retained_for_audit=True)
    summary['cases'].append(item)
    proof = args.evidence / 'proof.json'
    proof.write_text(json.dumps(summary, indent=2)+'\n')
    proof.chmod(0o600)
    print(json.dumps(dict(case=case, exit_code=code, seconds=item['elapsed_seconds'],
                         owners=len(owners), remaining=len(remaining), unknown=len(unknown))), flush=True)
    assert not timed_out and not cleanup_unknown, 'timed-out case cannot qualify even after graceful exit'
    assert not remaining and not unknown, 'cleanup uncertain: preserve private state, do not retry'
    assert all(owner['session'] == process.pid for owner in owners), 'owner must belong to the exact unique case SID'
    assert len(owners) == 2 and drained == {o['group'] for o in owners}, 'both owned drains required'
    assert code == 0, 'case failed: preserve exact evidence before any changed retry'
assert len(summary['cases']) == 2
