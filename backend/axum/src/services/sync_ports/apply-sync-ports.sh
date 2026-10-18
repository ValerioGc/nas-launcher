#!/usr/bin/env bash
# Reconciles local-network Syncthing ports in the nas_sync firewall set.
# The launcher records active sync in /var/lib/nas-os/sync-ports/<id>.
# Only fixed TCP/UDP 22000 and discovery UDP 21027 are managed.
# Runs as root through nas-os-sync-ports.service after state changes and at startup.

set -euo pipefail

readonly REQUESTS=${NAS_SYNC_PORTS_DIR:-/var/lib/nas-os/sync-ports}
readonly PORTS='22000, 21027'

fail() {
  printf 'sync ports not applied: %s\n' "$1" >&2
  exit 1
}

command -v nft >/dev/null || fail 'nft is not installed'

on=()
shopt -s nullglob
for file in "$REQUESTS"/*; do
  name=${file##*/}
  [[ $name =~ ^[A-Za-z0-9_-]{1,64}$ ]] || fail "$name is not the request of an application"
  [[ $(<"$file") == on ]] || fail "$name: the request says neither on nor anything else"
  on+=("$name")
done

nft flush set inet nas_base nas_sync || fail 'the set nas_sync is missing from the base firewall'
if ((${#on[@]} == 0)); then
  echo 'no sync on: ports closed'
  exit 0
fi
nft add element inet nas_base nas_sync "{ $PORTS }" || fail 'nft refused the ports'
echo "sync on for ${on[*]}: TCP and UDP $PORTS open to the local network"
