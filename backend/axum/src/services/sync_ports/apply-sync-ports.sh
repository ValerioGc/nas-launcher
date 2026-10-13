#!/usr/bin/env bash
#
#   apply-sync-ports.sh
#
# Opens the ports of the sync (Syncthing: TCP and UDP 22000, UDP 21027 for the local discovery) to
# the local network while an application has a sync on, and closes them otherwise, through the set
# `nas_sync` of the base firewall, which accepts only the private and link-local addresses.
#
# The launcher writes `on` in /var/lib/nas-os/sync-ports/<id> for each application with a sync on,
# and removes the file when it has none. This script reads the folder and puts the fixed ports in
# the set, or empties it: the ports are decided here, never by the file. It runs as root from
# nas-os-sync-ports.service, which the launcher starts after a change and at its own start, since
# the set is empty after a boot.

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
