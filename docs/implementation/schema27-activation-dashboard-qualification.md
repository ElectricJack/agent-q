# Assembled schema27, policy startup and live dashboard

Source `077a0adc2e5672fc37b9251a299343be4e540903` combines the ordered
schema/native preparation checkpoint with explicitly configured playbook owner
startup. The assembly built with locked/offline dependencies and passed 45
selected server tests across seven targets, with no failures or ignores. Strict
all-feature lint for the affected library/test targets and workspace formatting
passed. This is focused integration qualification, not a repeated full suite.

The isolated loopback dashboard preview upgraded its actual fixture database
from format25 to27. Its single synthetic attempt and exact fixture session
survived. All four actual-daemon browser checks passed in3.0s: navigation,
unsupported-feature behavior, narrow-screen settings, and roster/detail/deep-link
session inspection. Dashboard source `dbbba20936a9ff9c8767796832aa29b9a0523368`
and the existing verified bundle were unchanged. The preview remains at
`http://127.0.0.1:4188` with development authentication and no running agents.

The playbook owner remains unconfigured in that preview. Its separate
[activation evidence](AQ-14-owner-activation-evidence.md) proves explicit
startup/drive/restart/shutdown with disposable PostgreSQL and a synthetic Unix
transport peer. Ordinary TCP-only PostgreSQL deployments cannot enable that
initial Unix-only profile. Neither migration presence nor the dashboard enables
an engine implicitly.

[Exact commands, source/binary/bundle hashes and logs](evidence/schema27-activation-dashboard-077a0ad/artifact-sha256.json)
retain this assembly separately from the earlier dashboard smoke. The private
fixture backup stays in the local temporary directory; it is not part of the
published evidence. An exploratory request to a wrong bootstrap URL returned404;
the contract endpoint `/api/v1/bootstrap` returned200, and the browser suite used
the correct endpoint. No provider, native execution, production deployment or
network publication occurred. Native offers/grants, complete worker execution,
remaining dashboard domains and product gates remain open.
