#!/usr/bin/env bash
#
# Applies the name of the NAS chosen in the settings as the static hostname of the machine.
#
# It runs as root from `nas-os-hostname.service`, which the launcher starts once it has written the
# name next to its database. The name is read again as if anybody had written it: only a valid
# hostname goes to hostnamectl, and a refusal is left in hostname.error for the launcher to report.
# Then the gateway restarts, so its HTTPS certificate carries the new name.

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

# The gateway answers HTTPS with a certificate that names the machine, issued again for the new
# name when it starts. Not waited for: the request that changed the name came through it.
systemctl try-restart --no-block nas-os-gateway.service || true
