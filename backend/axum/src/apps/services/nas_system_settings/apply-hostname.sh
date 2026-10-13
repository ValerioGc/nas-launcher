#!/usr/bin/env bash
#
# Applies the name of the NAS chosen in the settings as the static hostname of the machine.
#
# It runs as root from `nas-os-hostname.service`, which the launcher starts once it has written the
# name next to its database. The name is read again as if anybody had written it: only a valid
# hostname goes to hostnamectl, and a refusal is left in hostname.error for the launcher to report.
# Then the gateway restarts, so its HTTPS certificate carries the new name.

set -euo pipefail

readonly request=/var/lib/nas-os/hostname
readonly error=/var/lib/nas-os/hostname.error
rm -f "$error"

name=$(<"$request") || name=''
if [[ ! $name =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then
  printf '%s\n' 'invalid NAS hostname' >"$error"
  exit 1
fi

if ! hostnamectl hostname "$name" 2>"$error"; then
  exit 1
fi
rm -f "$error"

# The gateway answers HTTPS with a certificate that names the machine, issued again for the new
# name when it starts. Not waited for: the request that changed the name came through it.
systemctl try-restart --no-block nas-os-gateway.service || true
