#!/bin/sh
# Off-host encrypted backup for the AQ VPS (E5). Installed as
# /usr/local/sbin/aq-backup and run by aq-backup.timer as root, with the
# restic repository and keys from /etc/agent-q/backup.env (root 0600).
#
# Dumps aq_server and aq_graph (pg_dump -Fc through the Compose postgres
# service), then backs up the dumps plus the host configuration (vault, env
# files, updater.toml, secrets) to restic, then applies retention. Any failed
# dump fails the run before restic is called; plaintext dumps never outlive it.
set -eu

: "${RESTIC_REPOSITORY:?set RESTIC_REPOSITORY (from /etc/agent-q/backup.env)}"
config=${AQ_CONFIG_DIR:-/etc/agent-q}
staging=${AQ_BACKUP_STAGING:-/var/lib/agent-q/backup-staging}
compose_dir=${AQ_COMPOSE_DIR:-/opt/agent-q}
state_dir=${AQ_UPDATER_STATE_DIR:-/var/lib/agent-q/updater}

compose() {
  docker compose --project-name agent-q --project-directory "$compose_dir" \
    -f "$compose_dir/compose.yaml" --env-file "$config/agent-q.env" \
    --env-file "$state_dir/release.env" "$@"
}

umask 077
mkdir -p "$staging"
trap 'rm -f "$staging"/*.dump' EXIT

for db in aq_server aq_graph; do
  compose exec -T postgres pg_dump -U agent_q -Fc "$db" >"$staging/$db.dump"
  [ -s "$staging/$db.dump" ] || { echo "aq-backup: empty dump of $db" >&2; exit 1; }
done

restic backup --tag aq --host aq-vps "$staging" "$config" "$compose_dir"
restic forget --tag aq --host aq-vps --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --prune
