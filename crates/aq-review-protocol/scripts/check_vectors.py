#!/usr/bin/env python3
"""Independent canonical SHA256 vectors; synthetic claims confer no authority."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
f = json.loads((root / "artifacts/fixtures.json").read_text())
def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                     separators=(",", ":")).encode()).hexdigest()

for body, artifact in (("subject", "subject_artifact"),
                       ("decision_receipt", "decision_artifact"),
                       ("check_receipt", "check_artifact")):
    assert digest(f[body]) == f[artifact]["digest"], body
assert digest(f["decision_request"]) == f["decision_receipt"]["request_digest"]
upstream = json.loads((root / "artifacts/upstream-result-record-v1.fixture.json").read_text())
assert digest(upstream["receipt"]) == f["subject"]["result"]["record_receipt"]["digest"]
assert digest(upstream["report"]) == f["subject"]["result"]["report_digest"]
print("6 independent canonical digest vectors passed")
