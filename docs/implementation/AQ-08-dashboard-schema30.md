# Schema30 dashboard integration

The local preview now serves the existing dashboard from daemon source `94893a3`
with ordered state30. The disposable fixture database upgraded from28 to30 after
a private archive was captured. Its fixture session remains available in the
roster, exact detail view and refreshed deep link. All four actual-daemon browser
checks passed in2.2seconds; the daemon build took33.77seconds.

[Exact binary and log evidence](AQ-08-dashboard-schema30.json) pins the rebuilt
daemon independently of its development readiness label. The preview is available
at <http://127.0.0.1:4188>. The archive stays outside the repository with mode0600.

This validates the local dashboard/database upgrade. Optional native owners and
playbooks remain inactive; no agent or model was launched. Terminal interaction
and unsupported domain views remain capability gated.
