#!/usr/bin/env python3
"""Check the command surface contract: schema validity and fixtures."""
import json
from pathlib import Path
import jsonschema

root = Path(__file__).resolve().parent
schema = json.loads((root / 'schema.json').read_text())
jsonschema.Draft202012Validator.check_schema(schema)
def validator(name):
    return jsonschema.Draft202012Validator({**schema, '$ref': f'#/$defs/{name}'}, format_checker=jsonschema.FormatChecker())
refusal = validator('Refusal')
for path in sorted((root / 'fixtures').glob('refusal-*.json')):
    value = json.loads(path.read_text())
    refusal.validate(value)
    assert value['code'].replace('_', '-') in path.stem, path.name
    assert value['grant'] in value['message'] and value['command'] in value['message'], path.name
grant_set = validator('GrantSet')
for bad in ([{'operation': 'question.answer', 'scope': 'project:self'}],
            [{'operation': 'task.read', 'scope': 'project:example'}],
            [{'operation': 'task.read', 'scope': 'project:self', 'x': 1}]):
    assert not grant_set.is_valid(bad), bad
print('commands-1 contract OK')

# commands.json: generated from the registry (aq-server it_server commands::).
commands = json.loads((root / 'commands.json').read_text())
assert commands['contract'] == 'commands-1' and commands['version'] == 1
ops = set(schema['$defs']['Operation']['enum'])
names = [c['name'] for c in commands['commands']]
assert names == sorted(set(names)), 'command names are unique and sorted'
import hashlib
def digest(value):
    return 'sha256:' + hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()
for c in commands['commands']:
    assert c['cli'] == [p.replace('_', '-') for p in c['name'].split('.')], c['name']
    assert c['grant'] is None or c['owner_only'] == (c['grant'] not in ops), c['name']
    assert c['principals'], c['name']
    for principal, entry in c['principals'].items():
        assert principal in ('owner', 'supervisor', 'agent')
        if principal == 'agent':
            assert not c['owner_only'], c['name']
            assert entry['route']['path'].startswith(('/v1/agent/', '/api/v1/commands/')), c['name']
        if '$ref' not in entry['params_schema'] or len(entry['params_schema']) > 1:
            assert entry['params_digest'] == digest(entry['params_schema']), c['name']
routes = json.loads((root / 'routes.json').read_text())
assert {r['class'] for r in routes['routes']} <= set(routes['classes'])
print('commands-1 commands.json and routes.json OK')
