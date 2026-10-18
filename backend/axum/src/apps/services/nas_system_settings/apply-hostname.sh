#!/usr/bin/env bash
# Applies the validated NAS name as the system's static hostname.
# Reads the request written beside the launcher database and revalidates its syntax.
# Writes failures to hostname.error for the launcher to report.
# Runs as root through nas-os-hostname.service and restarts the gateway to renew
# its HTTPS certificate for the updated name.

set -euo pipefail

readonly state=${NAS_STATE_DIR:-/var/lib/nas-os}
readonly hosts=${NAS_HOSTS_FILE:-/etc/hosts}
readonly request=$state/hostname
readonly error=$state/hostname.error
readonly rollback=${NAS_CONFIGURATION_HELPER:-/usr/local/lib/nas-wizard/packages/os_configuration/scripts/uninstall.sh}
rm -f "$error"
temporary=$(mktemp)
diagnostic=$(mktemp)
previous_hosts=$(mktemp)
finish() {
  local status=$?
  if ((status != 0)); then
    chmod 0644 "$diagnostic"
    mv -fT -- "$diagnostic" "$error"
  else
    rm -f -- "$diagnostic"
  fi
  rm -f -- "$temporary" "$previous_hosts"
  exit "$status"
}
trap finish EXIT

name=$(<"$request") || name=''
if [[ ! $name =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then
  printf '%s\n' 'invalid NAS hostname' >"$diagnostic"
  exit 1
fi

[[ -x $rollback ]] || { printf '%s\n' 'system restoration helper is missing' >"$diagnostic"; exit 1; }
"$rollback" --check 2>"$diagnostic" || exit 1

# Keeps the local name resolvable without changing the other host entries.
cp -- "$hosts" "$previous_hosts"
awk -v name="$name" '
  $1 == "127.0.1.1" {
    if (!written) print "127.0.1.1\t" name;
    written = 1;
    next;
  }
  { print }
  END { if (!written) print "127.0.1.1\t" name }
' "$hosts" >"$temporary"
previous=$(hostname)
if ! hostnamectl --static --transient hostname "$name" 2>"$diagnostic"; then
  exit 1
fi
if ! cat "$temporary" >"$hosts" 2>"$diagnostic"; then
  cat "$previous_hosts" >"$hosts" || true
  hostnamectl --static --transient hostname "$previous" || true
  exit 1
fi

# Queues the gateway restart so the hostname-change request can complete before certificate
# renewal.
systemctl try-restart --no-block nas-os-gateway.service || true
