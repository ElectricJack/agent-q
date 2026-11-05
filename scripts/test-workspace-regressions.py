"""Run the recorded Rust regressions inside aq test's foreground resource slot.

AQ_WORKSPACE_REGRESSION_SCOPE selects exact (default), area, workspace or checks.
AQ_WORKSPACE_REGRESSION_EVIDENCE names a new evidence directory for each run.
Requires the disposable DATABASE_URL and the pinned actual Graph fixture.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "docs/implementation/A3b-checks/persistent-failure-comparison.json"
# The recorded `<target>::<test>` names are module paths inside these grouped
# test binaries (crates/aq-server/tests/<binary>/main.rs).
BINARY = {
    "dashboard_aggregation": "it_dashboard",
    "native_accepted_preparation": "it_native",
    "native_cancellation": "it_native",
    "native_expiry": "it_native",
    "phase_execution_transport": "it_phase",
    "phase_never_granted": "it_phase",
    "phase_terminal_closure": "it_phase",
    "profiled_hot": "it_profiled",
    "schema30_preparation": "it_schema",
    "schema32_phase_execution": "it_schema",
    "schema33_phase_terminal": "it_schema",
    "schema43_cancellation": "it_schema",
}


def test_workspace_regressions():
    scope = os.environ.get("AQ_WORKSPACE_REGRESSION_SCOPE", "exact")
    assert scope in {"exact", "area", "workspace", "checks"}
    evidence = Path(os.environ["AQ_WORKSPACE_REGRESSION_EVIDENCE"])
    evidence.mkdir(parents=True, exist_ok=False)
    environment = os.environ.copy()
    environment["CARGO_BUILD_JOBS"] = "2"
    environment["RUST_TEST_THREADS"] = "1"
    # Workspace debug artifacts exhausted the shared filesystem before tests
    # started. Keep default optimization/features while bounding build storage.
    environment["CARGO_PROFILE_DEV_DEBUG"] = "0"
    environment["CARGO_PROFILE_TEST_DEBUG"] = "0"
    environment["CARGO_INCREMENTAL"] = "0"
    if scope != "checks":
        assert environment.get("DATABASE_URL"), "use scripts/test-db"
        binary = Path(environment["AQ_TEST_GRAPH_BINARY"])
        assert hashlib.sha256(binary.read_bytes()).hexdigest() == environment["AQ_TEST_GRAPH_SHA256"]
    baseline = json.loads(BASELINE.read_text())
    expected = [case["test"] for case in baseline]
    assert len(expected) == len(set(expected)) == 19
    targets = sorted({case["test"].split("::", 1)[0] for case in baseline})
    commands = []
    if scope == "exact":
        build = ["cargo", "test", "--locked", "-p", "aq-server", "--no-run"]
        for binary in sorted({BINARY[target] for target in targets}):
            build += ["--test", binary]
        commands.append(("compile", build))
        for case in baseline:
            target = case["test"].split("::", 1)[0]
            commands.append((case["test"], [
                "cargo", "test", "--locked", "-p", "aq-server", "--test", BINARY[target],
                case["test"], "--", "--exact", "--test-threads=1", "--nocapture",
            ]))
    elif scope == "area":
        command = ["cargo", "test", "--locked", "-p", "aq-server", "--no-fail-fast"]
        area = sorted(set(targets) | {"dashboard_aggregation", "native_cancellation", "phase_never_granted", "native_expiry"})
        for binary in sorted({BINARY[target] for target in area}):
            command += ["--test", binary]
        commands.append(("area", command + ["--", "--test-threads=1"] + [f"{target}::" for target in area]))
    elif scope == "workspace":
        commands.append(("workspace", [
            "cargo", "test", "--locked", "--workspace", "--no-fail-fast",
            "--", "--test-threads=2",
        ]))
    else:
        commands = [
            ("format", ["cargo", "fmt", "--check"]),
            ("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"]),
        ]
    results = {
        "scope": scope,
        "source": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD"], cwd=ROOT)).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "resource_controls": {key: environment[key] for key in (
            "CARGO_BUILD_JOBS", "RUST_TEST_THREADS", "CARGO_PROFILE_DEV_DEBUG",
            "CARGO_PROFILE_TEST_DEBUG", "CARGO_INCREMENTAL",
        )},
        "graph_sha256": environment.get("AQ_TEST_GRAPH_SHA256"),
        "expected_regressions": expected,
        "runs": [],
    }
    if scope != "checks":
        database = urlsplit(environment["DATABASE_URL"])
        results["database_endpoint"] = {"host": database.hostname, "port": database.port}
    for index, (name, command) in enumerate(commands):
        log = evidence / f"{index:02d}-{name.split('::')[0]}.txt"
        started = time.monotonic()
        with log.open("w") as output:
            completed = subprocess.run(command, cwd=ROOT, env=environment, stdout=output, stderr=subprocess.STDOUT)
        body = log.read_text()
        outcomes = {}
        grouped = "::" in name
        for line in body.splitlines():
            heading = re.search(r"Running tests/(\w+)/main\.rs ", line)
            if heading:
                grouped = heading[1] in BINARY.values()
            result = re.fullmatch(r"test (\S+) \.\.\. (ok|FAILED|ignored)", line)
            if result and grouped:
                case = result[1]
                if case in expected:
                    outcomes[case] = result[2]
        required = [name] if name in expected else expected if scope in {"area", "workspace"} else []
        results["runs"].append({
            "case": name, "command": command, "exit_code": completed.returncode,
            "seconds": round(time.monotonic() - started, 3), "log": log.name,
            "test_results": [line for line in body.splitlines() if line.startswith("test result:")],
            "regression_outcomes": outcomes,
            "missing_or_failed_regressions": [case for case in required if outcomes.get(case) != "ok"],
        })
        (evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(f"{name}: exit {completed.returncode}; {log}", flush=True)
        if name == "compile" and completed.returncode:
            break
    failures = [run for run in results["runs"] if run["exit_code"] or run["missing_or_failed_regressions"]]
    assert not failures, json.dumps(failures, indent=2)
