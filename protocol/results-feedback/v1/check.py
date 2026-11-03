"""Wire fixtures and app-level invariants; retained provenance needs SQL tests."""
import hashlib
import json
from pathlib import Path
from uuid import UUID
import jsonschema

ROOT = Path(__file__).parent
schema = json.loads((ROOT / 'schema.json').read_text())
format_checker = jsonschema.FormatChecker()

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()

def validate(kind, value):
    document = dict(schema, **{'$ref': '#/$defs/' + kind})
    document.pop('anyOf', None)
    jsonschema.Draft202012Validator(document, format_checker=format_checker).validate(value)
    if 'operation_id' in value:
        assert UUID(value['operation_id']).int
        assert 0 < int(value['expected_version']) <= 2**63 - 1
    if kind in ('Comment', 'Correction'):
        assert value['body'].strip() and 1 <= len(value['body'].encode()) <= 16384
    if kind == 'Comment':
        a = value['anchor']
        if a['kind'] == 'image_region':
            assert a['width'] > 0 and a['height'] > 0
            assert a['x'] + a['width'] <= 1 and a['y'] + a['height'] <= 1
        assert a['kind'] != 'video_range', 'unsupported_media_kind'
    if kind == 'FeedbackDraft':
        assert value['title'].strip() and 1 <= len(value['title']) <= 200
        assert not any(ord(c) < 32 for c in value['title'])
        assert 1 <= len(value['description'].encode()) <= 32768
        assert 1 <= len(value['acceptance_criteria'].encode()) <= 8192
        assert int(value['expected_project_revision']) > 0
    assert len(canonical(value)) <= 65536

for fixture in json.loads((ROOT / 'fixtures/valid.json').read_text()):
    validate(fixture['type'], fixture['value'])
for fixture in json.loads((ROOT / 'fixtures/invalid.json').read_text()):
    try:
        validate(fixture['type'], fixture['value'])
    except (AssertionError, ValueError, KeyError, jsonschema.ValidationError):
        continue
    raise AssertionError('invalid fixture accepted: ' + fixture['name'])
replay = json.loads((ROOT / 'fixtures/replay.json').read_text())
for key in ('original', 'identical', 'changed'):
    validate('Comment', replay[key])
assert canonical(replay['original']) == canonical(replay['identical'])
assert canonical(replay['original']) != canonical(replay['changed'])
assert replay['original']['operation_id'] == replay['changed']['operation_id']
assert replay['identical_disposition'] == 'confirmed' and replay['changed_status'] == 409
assert replay['canonical_sha256'] == hashlib.sha256(canonical(replay['original'])).hexdigest()
compat = json.loads((ROOT / 'fixtures/compatibility.json').read_text())
read = json.loads((ROOT / '../../dashboard-read/v1/schema.json').resolve().read_text())
actions = json.loads((ROOT / '../../dashboard-actions/v1/schema.json').resolve().read_text())
jsonschema.validate(compat['read'], read)
jsonschema.validate(compat['action'], actions)
assert compat['read']['api_version'] == 'dashboard-read-1'
assert compat['action']['kind'] == 'result.comment'
print('Results feedback: valid, invalid, replay and compatibility fixtures passed')
