#!/usr/bin/env bash
# Sets the hardware RTC alarm from the launcher state file.
# Runs as root through nas-os-wake.service because the sysfs alarm requires root.
# Automatic power-on also requires firmware support and an enabled RTC wake setting.
# Writing the alarm alone does not enable that firmware setting.

set -uo pipefail

readonly REQUEST=${NAS_WAKE_REQUEST:-/var/lib/nas-os/wakealarm}
readonly RTC=${NAS_WAKE_RTC:-/sys/class/rtc/rtc0/wakealarm}

fail() {
  printf 'alarm not set: %s\n' "$1" >&2
  exit 1
}

[[ -r $REQUEST ]] || fail "no request in $REQUEST"
[[ -w $RTC ]] || fail "$RTC is not writable: this machine does not expose the clock alarm"

at=$(tr -dc '0-9' <"$REQUEST")
[[ -n $at ]] || fail "unreadable request in $REQUEST"
now=$(date +%s)
((at > now)) || fail "the time asked for has already passed ($at)"

echo 0 >"$RTC" || fail "cannot clear the previous alarm"
echo "$at" >"$RTC" || fail "cannot write $RTC"
printf 'alarm set for %s\n' "$(date -d "@$at" '+%F %H:%M:%S')"
