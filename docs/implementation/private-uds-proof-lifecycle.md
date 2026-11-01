# Private PostgreSQL proof lifecycle after promotion

The PostgreSQL instance under `/tmp/aq-private-uds-n8vgb39f` now serves the
canonical local AQ preview. Its `canonical-use.json` marker transfers lifecycle
ownership to that preview. It must not be stopped or removed as disposable proof
cleanup. The private canonical authority record identifies the current database,
vault, service process and retained rollback source; it contains no copied domain
state or alternative workflow authority.

`scripts/private-uds-postgres-proof.py stop` refuses any promoted-use marker,
including malformed metadata or a broken symlink, before accessing the container.
It also refuses a running instance containing databases outside the original
isolated proof scope. Inspection remains available. Stopping the canonical preview
or its database must use its coordinated service lifecycle and preserve its data;
removing a marker is not a safe shutdown procedure.

The three lifecycle guard tests run with
`python3 scripts/test-private-uds-postgres-proof.py`. They exercise refusal and
the original isolated cleanup behavior with no Docker or PostgreSQL mutations.
The canonical database was never stopped to test these guards.
