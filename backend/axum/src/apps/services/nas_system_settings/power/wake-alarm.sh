#!/usr/bin/env bash
#
# Writes into the alarm of the hardware clock the time the launcher left in its state directory. It
# runs as root from `nas-os-wake.service`, because that sysfs file is writable by root only
#
# With the machine off, the clock stays powered by the battery on the board: when the time comes it
# powers the machine on. If the BIOS has "wake on RTC" disabled, this file is still written but the
# machine does not power on: it is the one thing that has to be enabled by hand.

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
