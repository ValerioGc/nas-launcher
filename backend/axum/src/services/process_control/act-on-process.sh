#!/usr/bin/env bash
#
# Terminates a process, or restarts the service it runs in, for the process widget of the launcher:
#
#   act-on-process.sh <terminate|restart>-<pid>-<started>
#
# <started> is the start time of the process in clock ticks after boot, as /proc/<pid>/stat says
# it: a process that ended and left its id to another one does not match, and nothing is done.
# The launcher checks the same rules before starting this, but what may be done is decided here:
#
# - a thread of the kernel and the first process are neither terminated nor restarted;
# - a process of a unit in ESSENTIAL, which keeps the NAS working, is only restarted;
# - restarting needs a service: a login session or a scope has none.
#
# It runs as root from `nas-os-process@<instance>.service`, the only unit of its kind polkit lets
# the launcher start (65-nas-os-processes.rules). Terminating a NAS app stops its whole service;
# other processes receive SIGTERM. Restarting asks systemd, and waits unless it is the launcher or gateway,
# which carry the request that started this.

set -euo pipefail

readonly PROC=${NAS_PROC:-/proc}
# The same list as ESSENTIAL in actions.rs.
readonly ESSENTIAL=(
  init.scope
  nas-os.service
  nas-os-gateway.service
  dbus.service
  dbus-broker.service
  polkit.service
  'systemd-*.service'
  NetworkManager.service
  ssh.service
)
readonly HELPER='nas-os-process@'

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

instance=${1:-}
[[ $instance =~ ^(terminate|restart)-([1-9][0-9]{0,9})-([0-9]{1,20})$ ]] ||
  fail "not a request for a process: ${instance:-<empty>}"
readonly verb=${BASH_REMATCH[1]} pid=${BASH_REMATCH[2]} started=${BASH_REMATCH[3]}

stat=$(cat -- "$PROC/$pid/stat" 2>/dev/null) || fail "process $pid is no longer running"
# The name between the parentheses may hold spaces and parentheses itself: the fields start after
# the last one. The first of them is field 3 of proc(5).
read -r -a fields <<<"${stat##*) }"
parent=${fields[1]:-} start=${fields[19]:-}
[[ $start == "$started" ]] || fail "process $pid is no longer the one shown"
if ((pid <= 2 || parent == 2)); then
  fail "process $pid belongs to the kernel or starts the system: it is left alone"
fi

# The first part of the cgroup path that is a service or a scope.
[[ -r $PROC/$pid/cgroup ]] || fail "process $pid is no longer running"
unit=''
while IFS= read -r line; do
  [[ $line == 0::* ]] || continue
  IFS=/ read -r -a parts <<<"${line#0::}"
  for part in "${parts[@]}"; do
    if [[ $part == *.service || $part == *.scope ]]; then
      unit=$part
      break
    fi
  done
done <"$PROC/$pid/cgroup"

is_essential() {
  local pattern
  for pattern in "${ESSENTIAL[@]}"; do
    # shellcheck disable=SC2053 # the pattern is a glob on purpose
    [[ $1 == $pattern ]] && return 0
  done
  return 1
}

case $verb in
  terminate)
    if [[ -n $unit ]] && is_essential "$unit"; then
      fail "$unit keeps the NAS working: it can only be restarted"
    fi
    if [[ $unit =~ ^nas-app@([a-zA-Z0-9_-]{1,64})\.service$ ]]; then
      systemctl stop -- "$unit"
    else
      kill -TERM -- "$pid"
    fi
    echo "process $pid${unit:+ of $unit}: terminated"
    ;;
  restart)
    [[ $unit == *.service && $unit != "$HELPER"* ]] ||
      fail "process $pid is not in a service that can be restarted"
    case $unit in
      nas-os.service | nas-os-gateway.service) systemctl restart --no-block -- "$unit" ;;
      *) systemctl restart -- "$unit" ;;
    esac
    echo "$unit: restarted"
    ;;
esac
