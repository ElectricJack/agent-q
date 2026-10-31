#!/usr/bin/env python3
"""Optional disposable probe of the qualified image; ordinary conformance is offline.

Creates only uniquely named local Docker resources and synthetic credentials.
Never selects a user database, graph, or repository. Does not pull/publish images.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parent
IMAGE = 'aq-graph:graph01-225329cd6acf-pinned'
IMAGE_ID = 'sha256:807beba98dcd20b0ae335f09cae8c06ecb182a352ee78de278cb47fccb2ee3be'
SOURCE = '225329cd6acf5a775fd2a0b016dd1c45fb16c79b'


def uid(n):
    return str(uuid.UUID(int=n))


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT).strip()


def main():
    assert run('docker', 'image', 'inspect', IMAGE, '--format', '{{.Id}}') == IMAGE_ID
    prefix = 'aq-api01-' + uuid.uuid4().hex[:12]
    network = prefix + '-network'
    pg = prefix + '-postgres'
    graph = prefix + '-graph'
    created = []
    owned_network = False
    with tempfile.TemporaryDirectory(prefix=prefix + '-') as temp:
        token_file = Path(temp) / 'admin-token'
        token_file.write_text(secrets.token_hex(32))
        token_file.chmod(0o600)
        admin = token_file.read_text()
        writer = secrets.token_hex(32)
        password = secrets.token_hex(24)
        workspace = uid(105)
        try:
            run('docker', 'network', 'create', network)
            owned_network = True
            run('docker', 'run', '-d', '--rm', '--name', pg, '--network', network,
                '-e', 'POSTGRES_USER=graph_fixture', '-e', 'POSTGRES_DB=graph_fixture',
                '-e', 'POSTGRES_PASSWORD=' + password, 'postgres:17-bookworm')
            created.append(pg)
            for _ in range(100):
                result = subprocess.run(['docker', 'exec', pg, 'pg_isready', '-U', 'graph_fixture'],
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if result.returncode == 0:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('isolated PostgreSQL did not become ready')
            run('docker', 'run', '-d', '--rm', '--name', graph, '--network', network,
                '--user', f'{os.getuid()}:{os.getgid()}', '-p', '127.0.0.1::8741',
                '--mount', f'type=bind,src={token_file},dst=/run/admin-token,readonly',
                '-e', f'DATABASE_URL=postgres://graph_fixture:{password}@{pg}:5432/graph_fixture',
                '-e', 'AQ_GRAPH_BIND=0.0.0.0:8741', '-e', 'AQ_GRAPH_BOOTSTRAP_WORKSPACE=' + workspace,
                '-e', 'AQ_GRAPH_BOOTSTRAP_ACTOR=api01-admin', '-e', 'AQ_GRAPH_BOOTSTRAP_TOKEN_FILE=/run/admin-token', IMAGE)
            created.append(graph)
            address = run('docker', 'port', graph, '8741/tcp')
            assert address.startswith('127.0.0.1:') and '\n' not in address
            base = 'http://' + address
            def http(path, body=None, token=admin, expected=200, scoped=True):
                url = base + (f'/v1/workspaces/{workspace}' if scoped else '') + path
                data = None if body is None else json.dumps(body).encode()
                req = urllib.request.Request(url, data=data, headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
                try:
                    with urllib.request.urlopen(req, timeout=10) as reply:
                        code, value = reply.status, json.load(reply)
                except urllib.error.HTTPError as reply:
                    code, value = reply.code, json.load(reply)
                if code != expected:
                    raise RuntimeError(f'fixture endpoint returned {code}, expected {expected}')
                return value
            for _ in range(100):
                try:
                    http('/health/ready', scoped=False)
                    break
                except (OSError, RuntimeError):
                    time.sleep(0.1)
            else:
                raise RuntimeError('isolated graph did not become ready')
            def mutate(key, operations, token=admin, expected=200):
                return http('/mutations', {'idempotency_key': key, 'reason': 'API-01 isolated contract probe', 'operations': operations}, token, expected)
            def node(n):
                return http('/nodes/' + uid(n))
            http('/credentials', {'actor_id': 'api01-writer', 'token': writer, 'capability': 'write'})
            mutate('seed-admin', [
                {'op': 'create_node', 'id': uid(1), 'kind': 'task', 'title': 'Implement scoped graph access', 'body': 'Keep execution authority separate.', 'task_state': 'active'},
                {'op': 'create_node', 'id': uid(3), 'kind': 'task', 'title': 'Add a bounded conformance fixture', 'task_state': 'active'},
                {'op': 'create_node', 'id': uid(4), 'kind': 'knowledge', 'title': 'UNAUTHORIZED-CANARY', 'body': 'Hidden graph content must not be projected.'},
                {'op': 'create_node', 'id': uid(5), 'kind': 'knowledge', 'title': 'Foreign-authored finding', 'body': 'Owned by another graph principal.'},
                {'op': 'create_node', 'id': uid(6), 'kind': 'task', 'title': 'Protected retired task', 'task_state': 'draft'}])
            mutate('seed-writer', [{'op': 'create_node', 'id': uid(2), 'kind': 'knowledge', 'title': 'Operation receipts survive response loss',
                                   'body': 'Preserve the same operation and graph idempotency key.', 'content_type': 'note', 'labels': ['recovery']}], writer)
            guard = {'op': 'update_node', 'id': uid(2), 'expected_version': 1, 'patch': {}}
            add = {'op': 'add_edge', 'id': uid(21), 'source': uid(2), 'target': uid(1), 'edge_type': 'references', 'target_revision': 1}
            before = node(2)
            receipt = mutate('guard-valid', [guard, add], writer)
            after = node(2)
            assert receipt['results'][0]['version'] == before['version'] == 1
            assert after['version'] == 2
            assert mutate('guard-valid', [guard, add], writer) == receipt
            stale = mutate('guard-stale', [guard, {**add, 'id': uid(24), 'target': uid(3)}], writer, 409)
            assert stale['error']['code'] == 'conflict' and node(2) == after
            foreign = mutate('guard-foreign', [{'op': 'update_node', 'id': uid(5), 'expected_version': 1, 'patch': {}},
                {**add, 'id': uid(25), 'source': uid(5)}], writer, 403)
            assert foreign['error']['code'] == 'forbidden' and node(5)['version'] == 1
            mutate('retire-protected', [{'op': 'update_node', 'id': uid(6), 'expected_version': 1,
                                        'patch': {'retired': True, 'retirement_reason': 'isolated fixture'}}])
            protected = mutate('guard-protected', [{'op': 'update_node', 'id': uid(6), 'expected_version': 2, 'patch': {}},
                {**add, 'id': uid(26), 'source': uid(6)}], writer, 403)
            assert protected['error']['code'] == 'forbidden' and node(6)['version'] == 2
            mutate('seed-context', [
                {'op': 'add_edge', 'id': uid(22), 'source': uid(1), 'target': uid(3), 'edge_type': 'parent-child'},
                {'op': 'add_edge', 'id': uid(23), 'source': uid(1), 'target': uid(4), 'edge_type': 'references', 'target_revision': 1}])
            raw_context = http('/nodes/' + uid(1) + '/context?depth=1&limit=64&max_bytes=1048576')
            assert all(e['id'] not in {uid(24), uid(25), uid(26)} for e in raw_context['edges'])
            nodes = {uid(n): node(n) for n in range(1, 7)}
            edges = {e['id']: e for e in raw_context['edges']}
            evidence = {'origin': {'repository': 'aq-graph', 'source_revision': SOURCE, 'image': IMAGE, 'image_id': IMAGE_ID,
                         'postgres_image_id': run('docker', 'image', 'inspect', 'postgres:17-bookworm', '--format', '{{.Id}}'),
                         'captured_at': datetime.now(timezone.utc).isoformat(),
                         'method': 'Public HTTP responses from isolated qualified image; synthetic records and credentials only.'},
                        'nodes': nodes, 'edges': edges, 'context': raw_context,
                        'revision': http('/nodes/' + uid(1) + '/revisions/1'),
                        'history': http('/nodes/' + uid(1) + '/history?limit=64'),
                        'mutation_receipt': receipt,
                        'guard_results': {'valid_same_transaction': 200, 'identical_replay': True,
                                          'guard_noop_does_not_advance_version': True, 'stale_guard_atomic_refusal': 409,
                                          'foreign_author_refused': 403, 'retired_source_refused': 403,
                                          'failed_edges_absent': True}}
            (ROOT / 'fixtures/qualified-graph.json').write_text(json.dumps(evidence, indent=2) + '\n')
            print(json.dumps({'result': 'passed', 'source_revision': SOURCE, 'image_id': IMAGE_ID,
                              'guard_results': evidence['guard_results']}, indent=2))
        finally:
            failures = []
            for container in reversed(created):
                result = subprocess.run(['docker', 'rm', '-fv', container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if result.returncode:
                    failures.append(container)
            if owned_network:
                result = subprocess.run(['docker', 'network', 'rm', network], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if result.returncode:
                    failures.append(network)
            if failures:
                raise RuntimeError('owned fixture resource cleanup failed')


if __name__ == '__main__':
    main()
