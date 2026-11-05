#!/usr/bin/env python3
"""MS1: create graph tasks in the aq-ms1 workspace (optionally A blocks B).

usage: ms1-task.py ID TITLE BODY [--blocks OTHER_ID]
       ms1-task.py --show ID...
"""
import json, os, sys, urllib.request
from pathlib import Path
BASE = Path(os.environ.get("AQ_MS1_HOME", Path.home() / ".local/share/aq-ms1"))
GRAPH = os.environ.get("AQ_DEV_GRAPH_URL", "http://127.0.0.1:28741")  # set by ms1-demo.sh per stack
URL = GRAPH + "/v1/workspaces/e0000000-0000-4000-8000-000000000001"
TOKEN = (BASE / "secrets/graph_token").read_text().strip()

def call(path, body=None):
    req = urllib.request.Request(URL + path, data=None if body is None else json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json", "Authorization": "Bearer " + TOKEN})
    with urllib.request.urlopen(req, timeout=15) as r:
        return json.load(r)

if sys.argv[1] == "--show":
    ready = {i["id"] for i in call("/ready")["items"]}
    for n in sys.argv[2:]:
        x = call("/nodes/" + n); x = x.get("node", x)
        print(json.dumps({"id": n, "title": x["title"], "task_state": x["task_state"], "version": x["version"],
                          "ready": n in ready}))
    sys.exit(0)
ident, title, body = sys.argv[1:4]
ops = [dict(op="create_node", id=ident, kind="task", task_state="active", title=title, body=body)]
if "--blocks" in sys.argv:
    ops.append(dict(op="add_edge", source=ident, target=sys.argv[sys.argv.index("--blocks") + 1], edge_type="blocks"))
m = call("/mutations", dict(idempotency_key="ms1-task-" + ident, reason="MS1 demo task", operations=ops))
print(json.dumps({"id": ident, "change_set_id": m.get("change_set_id")}))
