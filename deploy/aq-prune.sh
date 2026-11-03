#!/bin/sh
# Daily disk pruning for the AQ VPS (E5). Installed as /usr/local/sbin/aq-prune
# and run by aq-prune.timer as root.
#
# Always: remove stopped sandbox containers (their writable layers hold the
# per-attempt Cargo/target caches) and dangling sandbox and engine images.
# Above AQ_PRUNE_THRESHOLD percent use of AQ_PRUNE_PATH: also remove every
# unused sandbox image and every sandbox volume no container references.
#
# Engine (main Docker) images are only ever pruned when dangling: tagged and
# in-use images, including the updater's rollback image, are never removed.
set -eu

threshold=${AQ_PRUNE_THRESHOLD:-70}
path=${AQ_PRUNE_PATH:-/}
socket=${AQ_SANDBOX_SOCKET:?set AQ_SANDBOX_SOCKET to the rootless sandbox daemon socket}

sb() { docker -H "unix://$socket" "$@"; }

use=$(df --output=pcent "$path" | tail -n 1 | tr -dc '0-9')
echo "aq-prune: $path at ${use}% (deep prune above ${threshold}%)"

sb container prune -f
sb image prune -f
docker image prune -f

if [ "$use" -gt "$threshold" ]; then
  echo "aq-prune: deep prune of the sandbox daemon"
  sb image prune -a -f
  sb volume ls -q -f dangling=true | while read -r volume; do
    [ -n "$volume" ] && sb volume rm "$volume"
  done
fi

df -h "$path"
