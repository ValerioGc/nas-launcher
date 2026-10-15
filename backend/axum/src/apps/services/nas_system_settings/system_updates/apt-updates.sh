#!/usr/bin/env bash
#
# Refreshes the Debian package index or installs the upgrades the launcher chose. It runs as root
# from `nas-os-apt@refresh.service` and `nas-os-apt@upgrade.service`.
#
# apt-updates.sh refresh|upgrade

set -euo pipefail

readonly STATE=${NAS_STATE_DIR:-/var/lib/nas-os}
readonly SELECTION=$STATE/apt-upgrade
export DEBIAN_FRONTEND=noninteractive
# Another apt may hold the lock of dpkg (apt-daily, an upgrade from the settings): wait for it.
readonly LOCK_WAIT=(-o DPkg::Lock::Timeout=300)

fail() {
  printf 'system update failed: %s\n' "$1" >&2
  exit 1
}

case ${1:-} in
  refresh)
    apt-get update -q -o APT::Update::Error-Mode=any "${LOCK_WAIT[@]}" || fail 'the package index could not be refreshed'
    ;;
  upgrade)
    [[ -r $SELECTION ]] || fail "no selection in $SELECTION"
    mapfile -t packages < <(grep -E '^[a-z0-9][a-z0-9+.-]{0,127}$' "$SELECTION")
    rm -f "$SELECTION"
    [[ ${#packages[@]} -gt 0 ]] || fail 'the selection holds no package name'
    apt-get install -q -y --only-upgrade -o Dpkg::Options::=--force-confold "${LOCK_WAIT[@]}" \
      -- "${packages[@]}" ||
      fail 'apt-get could not install the upgrades'
    ;;
  *)
    fail "unknown job: ${1:-none}"
    ;;
esac
