#!/usr/bin/env bash
#
#   apply-discovery.sh
#
# Opens the discovery of NAS Finder (UDP 39877) to the local network while the option of the
# security center is on, and closes it otherwise, through the set `nas_discovery` of the base
# firewall, which accepts only the private and link-local IPv4 addresses.
#
# The launcher writes `on` in /var/lib/nas-os/discovery while the option is on, and removes the
# file when it is off. This script reads it and puts the fixed port in the set, or empties it: the
# port is decided here, never by the file. It runs as root from nas-os-discovery.service, which the
# launcher starts after a change and at its own start, since the set is empty after a boot.

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
