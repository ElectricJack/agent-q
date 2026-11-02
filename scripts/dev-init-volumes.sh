#!/bin/sh
# Initialize only this Compose project's private volumes; never host ownership.
set -eu
umask 077
for directory in /vault /run/aq-dev /run/aq-git /var/lib/aq /var/lib/aq/git /var/lib/aq/source-staging; do
    install -d -o 10001 -g 10001 -m 0700 "$directory"
done
temporary=''
trap 'if [ -n "$temporary" ]; then rm -f "$temporary"; fi' EXIT
copy_private() {
    source=$1
    target=$2
    test ! -L "$target" || { echo 'refusing a symlink in retained development configuration' >&2; exit 1; }
    if [ -e "$target" ]; then
        cmp -s "$source" "$target" || { echo 'retained development configuration differs; preserve it and use a fresh Compose project' >&2; exit 1; }
        test "$(stat -c '%u:%g:%a' "$target")" = '10001:10001:600' || { echo 'retained development configuration ownership differs' >&2; exit 1; }
    else
        temporary=$(mktemp "$(dirname "$target")/.e0-init.XXXXXX")
        install -o 10001 -g 10001 -m 0600 "$source" "$temporary"
        ln "$temporary" "$target"
        rm -f "$temporary"
        temporary=''
    fi
}
copy_private /dev-vault/config.toml /vault/config.toml
copy_private /run/secrets/graph_token /run/aq-dev/graph_token
