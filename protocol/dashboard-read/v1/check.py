#!/usr/bin/env python3
"""Validate retained synthetic actual-route envelopes; no service connection."""
import json
from copy import deepcopy
from pathlib import Path
import jsonschema

root = Path(__file__).resolve().parent
schema = json.loads((root / "schema.json").read_text())
jsonschema.Draft202012Validator.check_schema(schema)
for name in ("bootstrap", "overview", "sessions", "session", "projects"):
    body = json.loads((root / "fixtures" / f"{name}.json").read_text())
    jsonschema.validate(body, schema)
    assert body["api_version"] == "dashboard-read-1"
    if name == "bootstrap":
        assert all(c["version"] == "1" for c in body["data"]["capabilities"])
        assert body["data"]["product"]["dashboard_digest"] is None
print("5 actual-route synthetic fixtures match dashboard-read-1 schema")

quota_names = ("empty", "nonempty", "partial", "stale", "unknown")
for name in quota_names:
    body = json.loads((root / "fixtures" / f"provider-usage-{name}.json").read_text())
    jsonschema.validate(body, schema)
    assert isinstance(body["data"], list)
valid = json.loads((root / "fixtures" / "provider-usage-nonempty.json").read_text())
for field, value in (("used_percent", -1), ("used_percent", 101), ("state", "surprising"), ("extra", True)):
    invalid = deepcopy(valid)
    invalid["data"][0]["windows"][0][field] = value
    assert not jsonschema.Draft202012Validator(schema).is_valid(invalid), field
print("5 quota examples and invalid quota fields checked")

# Owner-backed overview with a null decision transition time and exact digest pins.
overview = json.loads((root / "fixtures" / "overview-nonempty.json").read_text())
jsonschema.validate(overview, schema)
assert any(i["kind"] == "decision" and i["since"] is None and i["action"] is None
           for i in overview["data"]["attention"])
assert any(i["kind"] == "failed" and i["subject"]["revision"].startswith("sha256:")
           for i in overview["data"]["attention"])
for revision in ("0", "01", "x", "a" * 64, "sha256:" + "G" * 64):
    invalid = deepcopy(overview)
    invalid["data"]["attention"][0]["subject"]["revision"] = revision
    assert not jsonschema.Draft202012Validator(schema).is_valid(invalid), revision
invalid = deepcopy(overview)
invalid["data"]["attention"][0]["kind"] = "invented_kind"
assert not jsonschema.Draft202012Validator(schema).is_valid(invalid)
bootstrap = json.loads((root / "fixtures" / "bootstrap.json").read_text())
bootstrap["data"]["principal"]["authorization_revision"] = "sha256:" + "a" * 64
assert not jsonschema.Draft202012Validator(schema).is_valid(bootstrap)
print("Owner overview, null transition time, reference digest pins and strict numeric revisions checked")

search = json.loads((root / "fixtures" / "search.json").read_text())
jsonschema.validate(search, schema)
assert {g["type"] for g in search["data"]["groups"]} == {"task", "session", "result", "playbook", "knowledge"}
assert all(g["has_more"] == isinstance(g["next_cursor"], str) for g in search["data"]["groups"])
for revision in ("0", "01", "x", "a" * 64, "sha256:" + "G" * 64):
    invalid = deepcopy(search)
    invalid["data"]["groups"][0]["hits"][0]["ref"]["revision"] = revision
    assert not jsonschema.Draft202012Validator(schema).is_valid(invalid), revision
for field, value in (("type", "unknown"), ("has_more", "true"), ("extra", True)):
    invalid = deepcopy(search)
    invalid["data"]["groups"][0][field] = value
    assert not jsonschema.Draft202012Validator(schema).is_valid(invalid), field
print("Grouped search route fixture and invalid search revisions/types/fields checked")

# Aggregate/search examples include both known-empty and unavailable owners.
for route in ('overview', 'search'):
    for case in ('nonempty', 'empty', 'partial', 'unknown'):
        fixture = json.loads((root / 'fixtures' / f'{route}-{case}.json').read_text())
        jsonschema.validate(fixture, schema)
        sources = fixture['data']['sources']
        if case == 'unknown':
            assert all(s['observation'] is None and s['total']['value'] is None for s in sources)
        if case == 'empty':
            assert any(s['total'] == {'accuracy': 'exact', 'value': 0} for s in sources)
        if case == 'partial':
            assert any(s['coverage']['state'] in ('partial', 'unknown') for s in sources)
        if route == 'search' and case in ('empty', 'unknown'):
            assert all(not g['hits'] and not g['has_more'] and g['next_cursor'] is None for g in fixture['data']['groups'])
        if route == 'overview' and case in ('empty', 'unknown'):
            assert not fixture['data']['attention'] and not fixture['data']['recent_outcomes']
print('8 aggregate/search examples validate with empty and unknown semantics')
