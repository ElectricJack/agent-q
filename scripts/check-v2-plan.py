#!/usr/bin/env python3
"""Validate the repository's planning DAG and optionally render its Markdown view.

This does not call an AQ daemon, create issues, or execute implementation tasks.
"""

import argparse
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/plans/0001-v2-milestones.json"
OUTPUT = SOURCE.with_suffix(".md")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_plan():
    plan = json.loads(SOURCE.read_text())
    require(plan["schema_version"] == 1, "unsupported plan schema")
    tasks = plan["tasks"]
    by_id = {task["id"]: task for task in tasks}
    require(len(by_id) == len(tasks), "duplicate task IDs")
    milestones = {item["id"]: item for item in plan["milestones"]}
    require(len(milestones) == len(plan["milestones"]), "duplicate milestone IDs")
    requirements = set()
    for prefix, sources in plan["requirement_sources"].items():
        for relative in sources:
            path = SOURCE.parent / relative
            require(path.is_file(), f"missing requirement source: {relative}")
            requirements.update(re.findall(r"^\| (" + prefix + r"\d+) \|", path.read_text(), re.M))
    covered = set()
    for task in tasks:
        name = task["id"]
        require(re.fullmatch(r"[A-Z]+-\d{2}", name), f"invalid task ID: {name}")
        require(task["status"] in plan["status_values"], f"invalid status: {name}")
        require(task["milestone"] in milestones, f"invalid milestone: {name}")
        require((ROOT.parent / task["repo"]).is_dir(), f"missing owner repo/directory: {name}")
        require(task["scope"] and task["acceptance"], f"missing task contract: {name}")
        deps = task["depends_on"]
        require(len(deps) == len(set(deps)), f"duplicate dependencies: {name}")
        require(name not in deps, f"self dependency: {name}")
        require(set(deps) <= by_id.keys(), f"unknown dependency: {name}")
        require(set(task["requirements"]) <= requirements, f"unknown requirement: {name}")
        covered.update(task["requirements"])
        if task["status"] == "complete":
            require(task["evidence"], f"completed task lacks evidence: {name}")
            require(all(by_id[d]["status"] == "complete" for d in deps), f"incomplete prerequisite: {name}")
        for relative in task["evidence"]:
            require((SOURCE.parent / relative).exists(), f"missing evidence for {name}: {relative}")
    require(covered == requirements, f"unassigned requirements: {sorted(requirements - covered)}")

    visiting, visited, order = set(), set(), []

    def visit(name):
        require(name not in visiting, f"dependency cycle at {name}")
        if name in visited:
            return
        visiting.add(name)
        for dependency in by_id[name]["depends_on"]:
            visit(dependency)
        visiting.remove(name)
        visited.add(name)
        order.append(name)

    for name in by_id:
        visit(name)

    def ancestors(name):
        result = set()
        for dependency in by_id[name]["depends_on"]:
            result.add(dependency)
            result.update(ancestors(dependency))
        return result

    for milestone in milestones.values():
        gate = milestone["gate"]
        require(gate in by_id, f"unknown milestone gate: {gate}")
        require(by_id[gate]["milestone"] == milestone["id"], f"gate in wrong milestone: {gate}")
        required = {task["id"] for task in tasks if task["milestone"] == milestone["id"]}
        require(required <= ancestors(gate) | {gate}, f"milestone tasks not covered by gate: {milestone['id']}")
    final_gate = plan["milestones"][-1]["gate"]
    require(set(by_id) <= ancestors(final_gate) | {final_gate}, "tasks disconnected from final gate")
    for key in ("audit", "snapshot"):
        require((SOURCE.parent / plan[key]).is_file(), f"missing {key}")
    return plan, order, requirements


def ready_tasks(plan):
    by_id = {task["id"]: task for task in plan["tasks"]}
    return [task for task in plan["tasks"] if task["status"] == "planned"
            and all(by_id[dep]["status"] == "complete" for dep in task["depends_on"])]


def link(name):
    return f"[{name}](#{name.lower()})"


def render(plan):
    lines = [
        "# Agent Q v2 implementation milestones", "",
        f"**Planning baseline:** {plan['date']}", "",
        "This is the execution plan for the [implementation audit](" + plan["audit"] + "). "
        "The [JSON task graph](0001-v2-milestones.json) is the canonical planning source. "
        "This Markdown view is generated with `python3 scripts/check-v2-plan.py --render`; edit the JSON and regenerate it.", "",
        plan["format_note"], "",
        "## How to execute this plan", "",
        "Start with the ready tasks below. Before claiming a task, compare the "
        "[audit snapshot](" + plan["snapshot"] + ") with current work and identify any overlapping implementation. "
        "Qualify existing component code rather than rebuilding it. Pin contract/candidate revisions in each task's evidence.", "",
        plan["dependency_semantics"], "",
        "Each task has one owning repository. Contract work is reviewed by both endpoints, while integration gates are owned by agent-q. "
        "When a task is too broad after investigation, split it into bounded child tasks with explicit dependencies and keep the original gate dependent on them. "
        "A failed benchmark or newly discovered capability gap adds work to the graph; it is not grounds to mark a gate complete.", "",
        "The original planning baseline completed AUD-00; current statuses below record subsequent implementation and qualification. Existing code and recorded tests are reusable evidence, "
        "not proof that the new product integration tasks have already passed. No elapsed-time estimates or staffing assumptions are encoded.", "",
        "### Definition of done", "",
    ]
    lines += ["- " + item for item in plan["definition_of_done"]]
    lines += ["", "### Ready work", "", "| Task | Owner | Deliverable |", "| --- | --- | --- |"]
    lines += [f"| {link(task['id'])} | {task['repo']} | {task['title']} |" for task in ready_tasks(plan)]
    lines += ["", "The daemon authority track can start after BASE-01 and CON-01. The worker runtime track follows connector extraction. "
              "Graph, Git and playbook qualification are independent early tracks; they are not artificial prerequisites for fake-agent execution. "
              "Dashboard work waits for a versioned inspection/action API, but does not wait for production deployment.", "",
              "## Milestone outcomes", "", "| Milestone | Acceptance gate | Observable result |", "| --- | --- | --- |"]
    for milestone in plan["milestones"]:
        lines.append(f"| {milestone['id']}: {milestone['title']} | {link(milestone['gate'])} | {milestone['outcome']} |")
    lines += ["", "The milestone spine below is a summary. The task dependencies in the JSON are authoritative.", "",
              "```mermaid", "flowchart LR", '    M1[Contract and connector] --> M2[Fake-agent execution]',
              '    M2 --> M3[Recovery proof]', '    M3 --> M4[Real agent and retained source]',
              '    G[Graph and Git candidates] --> M4', '    M4 --> M5[Policy pilots]',
              '    P[Playbook candidate] --> M5', '    M5 --> M6[Product acceptance]',
              '    U[API and dashboard] --> M6', '    R[Manifest and upgrade work] --> M6', "```", "",
              "## Task graph", "", "Requirements name implementation responsibility, not passing test coverage. "
              "E refers to ecosystem acceptance; X to shared failure scenarios; D to daemon requirements; W to worker requirements; U to dashboard acceptance.", ""]
    for milestone in plan["milestones"]:
        lines += [f"### {milestone['id']} overview", "", "| Task | Owner | Hard prerequisites | Status |", "| --- | --- | --- | --- |"]
        for task in plan["tasks"]:
            if task["milestone"] != milestone["id"]:
                continue
            deps = ", ".join(link(dep) for dep in task["depends_on"]) or "None"
            lines.append(f"| {link(task['id'])}: {task['title']} | {task['repo']} | {deps} | {task['status']} |")
        lines.append("")
    lines += ["## Task contracts", ""]
    for task in plan["tasks"]:
        deps = ", ".join(link(dep) for dep in task["depends_on"]) or "None"
        lines += [f"### {task['id']}", "", f"**{task['title']}**", "",
                  f"Owner: `{task['repo']}`. Milestone: {task['milestone']}. Kind: {task['kind']}. Status: {task['status']}.", "",
                  f"Hard prerequisites: {deps}.", "", task["scope"], "", "Acceptance:", ""]
        lines += ["- " + item for item in task["acceptance"]]
        lines += ["", "Requirement links: " + ", ".join(task["requirements"]) + ".", ""]
        if task["out_of_scope"]:
            lines += ["Outside this task: " + "; ".join(task["out_of_scope"]) + ".", ""]
        if task["evidence"]:
            lines += ["Evidence: " + ", ".join(f"[{Path(item).name}]({item})" for item in task["evidence"]) + ".", ""]
    lines += ["## Validation and updates", "", "```sh", "python3 scripts/check-v2-plan.py",
              "python3 scripts/check-v2-plan.py --ready", "python3 scripts/check-v2-plan.py --render", "```", "",
              "The checker verifies unique IDs, known requirements, evidence references, acyclicity, hard prerequisites for completed tasks, "
              "milestone gate coverage, final-gate reachability and generated-document consistency. It never creates live AQ tasks or starts implementation.", "",
              "When implementation work is admitted to AQ later, retain these stable plan IDs as provenance. "
              "Use one task-status authority; explicitly replace this file's planning statuses with links to that execution system rather than maintaining two independently writable queues.", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--render", action="store_true", help="regenerate the Markdown plan")
    parser.add_argument("--ready", action="store_true", help="print ready task IDs as JSON")
    args = parser.parse_args()
    plan, order, requirements = read_plan()
    rendered = render(plan)
    if args.render:
        OUTPUT.write_text(rendered)
    else:
        require(OUTPUT.exists() and OUTPUT.read_text() == rendered, "Markdown plan is stale; run --render")
    if args.ready:
        print(json.dumps([task["id"] for task in ready_tasks(plan)]))
    else:
        print(f"PASS: {len(order)} tasks, {len(plan['milestones'])} milestones, "
              f"{len(requirements)} assigned requirements/scenarios; DAG and Markdown are consistent.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        sys.exit(1)
