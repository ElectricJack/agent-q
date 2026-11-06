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
