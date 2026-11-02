#!/usr/bin/env python3
"""Check the server-owned union, retained examples and replay relationships."""
import copy
import hashlib
import json
from pathlib import Path
from uuid import UUID
import jsonschema

root = Path(__file__).resolve().parent
schema = json.loads((root / 'schema.json').read_text())
read_schema = json.loads((root.parents[1] / 'dashboard-read/v1/schema.json').read_text())
jsonschema.Draft202012Validator.check_schema(schema)
validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker())
read_validator = jsonschema.Draft202012Validator(read_schema)
def load(name):
    return json.loads((root / 'fixtures' / (name + '.json')).read_text())
def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def validate_request(value):
    validator.validate(value)
    payload, subject = value['payload'], value['subject']
    assert UUID(value['operation_id']).int and payload['operation_id'] == value['operation_id']
    kind = value['kind']
    key = {'task.create':'project', 'decision.resolve':'decision', 'supervisor.message.send':'conversation'}[kind]
    revision = str(payload[{'task.create':'expected_project_revision', 'decision.resolve':'expected_decision_revision', 'supervisor.message.send':'expected_conversation_revision'}[kind]])
    assert value['expected_versions'] == {key:revision} and subject['revision'] == revision
    assert subject['type'] == key and UUID(subject['id']).int
    assert 0 < int(revision) <= 2**63 - 1
    if kind == 'task.create':
        assert payload['title'].strip() and len(payload['title']) <= 512
        assert len(payload['body'].encode()) <= 32768 and len(payload['acceptance_criteria'].encode()) <= 8192
        assert len(payload['labels']) <= 16 and len(set(payload['labels'])) == len(payload['labels'])
        assert all(s.strip() and len(s.encode()) <= 128 for s in payload['labels'])
    elif kind == 'supervisor.message.send':
        assert UUID(payload['message_id']).int and payload['message_id'] != value['operation_id']
        assert payload['body'].strip() and len(payload['body'].encode()) <= 16384
        assert payload['context'] == [] and payload['attachments'] == []
    else:
        assert payload['decision_id'] == subject['id'] and payload['schema_version'] == 1
        assert payload['expected_subject']['version'] == '1'
    assert len(canonical(value)) <= 65536

for name in ('task-create', 'decision-resolve', 'supervisor-message-send'):
    validate_request(load('request-' + name))
for disposition in ('pending', 'confirmed', 'conflict', 'rejected', 'unknown'):
    value = load('operation-' + disposition)
    validator.validate(value)
    assert value['api_version'] == 'dashboard-actions-1'
    operation = value['data']
    assert operation['disposition'] == disposition
    unresolved = disposition in ('pending', 'unknown')
    assert value['pending_operations'] == ([operation['operation_id']] if unresolved else [])
    assert operation['recovery'] == ('lookup_original' if unresolved else 'none' if disposition == 'confirmed' else 'refresh_and_review')
    if disposition != 'confirmed':
        assert not operation['receipt_refs'] and not operation['affected_entities']
    request = load('request-supervisor-message-send')
    assert operation['request_digest'] == 'sha256:' + hashlib.sha256(canonical({'request':request, 'scope':operation['scope']})).hexdigest()
for status in (401,403,404,409,410,413,422,426,429,503):
    error = load('error-' + str(status))
    validator.validate(error)
    assert error['retryable'] == (status in (429,503))
    if status == 404:
        assert 'operation_id' not in error
validator.validate(load('expired-cursor'))
assert load('expired-cursor')['code'] == 'cursor_restart_required'
replay = load('replay')
for key in ('original','identical','changed'):
    validate_request(replay[key])
assert canonical(replay['original']) == canonical(replay['identical'])
assert canonical(replay['original']) != canonical(replay['changed'])
assert all(replay[key]['operation_id'] == replay['original_operation_id'] for key in ('original','identical','changed'))
assert replay['identical_disposition'] == 'confirmed' and replay['changed_status'] == 409
for case in load('invalid'):
    try:
        validate_request(case['request'])
    except (AssertionError, jsonschema.ValidationError, ValueError, KeyError):
        continue
    raise AssertionError('invalid request accepted: ' + case['name'])
compat = load('compatibility')
read_validator.validate(compat['read'])
validator.validate(compat['action'])
assert not read_validator.is_valid(compat['read_with_action_version'])
assert not validator.is_valid(compat['action_with_read_version'])
for contract in compat['read']['data']['contracts']:
    path = root.parents[1] / contract['name'] / 'v1/schema.json'
    assert contract['digest'] == 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()
assert len(compat['read']['data']['contracts']) == 2
assert compat['read']['api_version'] == 'dashboard-read-1'
print('3 requests, 5 dispositions, 10 errors, cursor, replay, invalid and compatibility fixtures passed')
