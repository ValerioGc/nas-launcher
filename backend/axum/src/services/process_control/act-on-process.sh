#!/usr/bin/env bash
# Executes a validated process termination or service restart request.
# The instance identifies the action, PID, and start time in clock ticks since boot.
# The start time prevents a reused PID from matching another process.
# Kernel threads and PID 1 are excluded. Essential services can only restart.
# Restart requests require a service rather than a session or scope.
# Terminating a NAS application stops its unit. Other targets receive SIGTERM.
# Runs as root through nas-os-process@<instance>.service and repeats launcher checks.
# Launcher and gateway restarts are queued so the requesting connection can complete.

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
# Finds proc fields after the final closing parenthesis because process names can contain
# parentheses.
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
