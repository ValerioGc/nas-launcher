#!/usr/bin/env bash
# Reconciles local NAS Finder discovery on UDP 39877.
# Reads the marker at /var/lib/nas-os/discovery and updates the nas_discovery set.
# Only the fixed discovery port is managed.
# Runs as root through nas-os-discovery.service after changes and at startup.

set -euo pipefail

readonly REQUEST=${NAS_DISCOVERY_REQUEST:-/var/lib/nas-os/discovery}
readonly PORT=39877

fail() {
  printf 'discovery not applied: %s\n' "$1" >&2
  exit 1
}

command -v nft >/dev/null || fail 'nft is not installed'

on=false
if [[ -e $REQUEST ]]; then
  [[ $(<"$REQUEST") == on ]] || fail 'the request says neither on nor anything else'
  on=true
fi

nft flush set inet nas_base nas_discovery || fail 'the set nas_discovery is missing from the base firewall'
if ! $on; then
  echo 'discovery off: port closed'
  exit 0
fi
nft add element inet nas_base nas_discovery "{ $PORT }" || fail 'nft refused the port'
echo "discovery on: UDP $PORT open to the local network"
