#!/usr/bin/env python3
"""Developer-only shape/semantic validator. No HTTP, credentials or production policy."""
import argparse
import hashlib
from datetime import datetime
import re
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator, FormatChecker, ValidationError, validators

ROOT = Path(__file__).resolve().parent
FORMATS = FormatChecker()


@FORMATS.checks('date-time')
def rfc3339(value):
    if not isinstance(value, str):
        return True
    if not re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})', value):
        return False
    try:
        return datetime.fromisoformat(value.replace('Z', '+00:00')).tzinfo is not None
    except ValueError:
        return False


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def strict_loads(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    return json.loads(text, object_pairs_hook=pairs,
                      parse_float=lambda _: (_ for _ in ()).throw(ValueError('fractional JSON numbers unsupported')),
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON unsupported')))


def utf8_bytes(validator, limit, instance, schema):
    if isinstance(instance, str) and len(instance.encode('utf-8')) > limit:
        yield ValidationError(f'UTF-8 byte limit {limit} exceeded')


def int64_token(validator, enabled, instance, schema):
    if enabled and isinstance(instance, str) and instance.isdigit() and not 0 < int(instance) <= 2**63-1:
        yield ValidationError('version token is outside positive signed int64')


Validator = validators.extend(Draft202012Validator, {'x-utf8-maxBytes': utf8_bytes, 'x-int64-token': int64_token})
SCHEMA = strict_loads((ROOT / 'schema.json').read_text())


def validate(name, value, max_bytes=1048576):
    if name not in SCHEMA['$defs']:
        raise ValueError('unknown contract type')
    canonical_bytes = canonical(value)
    if len(canonical_bytes) > max_bytes:
        raise ValueError('serialized message byte budget exceeded')
    Validator({'$defs': SCHEMA['$defs'], '$ref': '#/$defs/' + name}, format_checker=FORMATS).validate(value)
    if name == 'ContextResponse':
        if digest(value['profile']) != value['binding']['profile_digest']:
            raise ValueError('immutable resolved profile digest mismatch')
        if digest(value['context']) != value['binding']['context_digest']:
            raise ValueError('immutable context digest mismatch')
        snapshot = value['context']
        node_ids = [n['id'] for n in snapshot['nodes']]
        if len(set(node_ids)) != len(node_ids) or not set(snapshot['root_ids']) <= set(node_ids):
            raise ValueError('context root/member identity mismatch')
        if any(e['source'] not in node_ids or e['target'] not in node_ids for e in snapshot['edges']):
            raise ValueError('context edge endpoint absent from authorized projection')
        if value['binding']['task_id'] not in snapshot['root_ids']:
            raise ValueError('bound task missing from launch context roots')
        read_scope = value['profile']['read_scope']
        if read_scope['mode'] == 'records' and not set(node_ids + [n['id'] for n in snapshot['pinned_revisions']]) <= set(read_scope['record_ids']):
            raise ValueError('launch context exceeds immutable read scope')
        if read_scope['mode'] == 'records' and not set(value['profile']['write_scope']['record_ids']) <= set(read_scope['record_ids']):
            raise ValueError('write scope is not readable')
        budget = value['profile']['retry_budget']
        if budget['initial_delay_ms'] > budget['max_delay_ms'] or budget['max_delay_ms'] > budget['max_elapsed_ms']:
            raise ValueError('retry timing bounds inconsistent')
        if len(canonical_bytes) > value['profile']['max_response_bytes']:
            raise ValueError('profile response budget exceeded')
    if name in ('CommandRequest',):
        proposals = [c for c in value['commands'] if c['kind'] == 'propose_decomposition']
        if proposals and len(value['commands']) != 1:
            raise ValueError('proposal must be the only command; live and draft transactions cannot mix')
    if name in ('RecoveryHint',):
        if value['after_ms'] > value['remaining_elapsed_ms']:
            raise ValueError('retry delay exceeds remaining elapsed budget')
        exhausted = value['remaining_attempts'] == 0 or value['remaining_elapsed_ms'] == 0
        if exhausted and (value['action'] != 'manual_reconciliation' or value['after_ms'] != 0):
            raise ValueError('exhausted retries require manual reconciliation, never a fresh ID')
    if name in ('OperationStatus', 'UnknownOperation', 'PendingOperation') and value.get('recovery'):
        validate('RecoveryHint', value['recovery'])
    if name == 'RecoveryResponse':
        for operation in value['operations']:
            validate('OperationStatus', operation)
            if any(operation['binding'][k] != value['attempt'][k] for k in value['attempt']):
                raise ValueError('recovery response crosses attempt binding')
    if name == 'ErrorResponse' and value['recovery'] is not None:
        validate('RecoveryHint', value['recovery'])
    if name in ('ConfirmedOperation', 'OperationStatus') and value.get('state') == 'confirmed':
        if value['receipt']['workspace_id'] != value['binding']['workspace_id']:
            raise ValueError('receipt workspace binding mismatch')
    return value


def upstream_validate(name, value):
    document = strict_loads((ROOT / 'upstream/aq-graph-openapi.json').read_text())
    Draft202012Validator({'components': document['components'], '$ref': '#/components/schemas/' + name},
                         format_checker=FORMATS).validate(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('type', help='public $defs name')
    parser.add_argument('file', help='JSON file or - for stdin')
    args = parser.parse_args()
    try:
        raw = sys.stdin.read() if args.file == '-' else Path(args.file).read_text()
        value = strict_loads(raw)
        validate(args.type, value)
    except (ValueError, OSError, ValidationError):
        # Do not echo a request body, private file contents or credentials.
        parser.exit(2, 'contract validation failed\n')
    print(json.dumps({'valid': True, 'type': args.type, 'canonical_sha256': digest(value)}, sort_keys=True))


if __name__ == '__main__':
    main()
