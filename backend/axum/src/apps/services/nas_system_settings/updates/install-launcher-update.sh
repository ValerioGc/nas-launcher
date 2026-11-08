#!/usr/bin/env bash
# Installs verified NAS releases in wizard, gateway and launcher order, and puts the previous
# version back when the NAS does not work afterwards.
# Runs as root through nas-os-update.service, with the archives the launcher staged.
#
# Before anything changes it records what works on the NAS. That is the launcher, the gateway on
# HTTP and on HTTPS, the addresses of the network interfaces and the default route, the base
# firewall and the way out to the internet. It stops the affected services and backs up every file
# the three installers may write. Each component then runs its own installer. Afterwards all that
# worked before has to work again. Otherwise the files the new version wrote or added are removed,
# the backup is extracted, the previous firewall is loaded again and the previous services start.
# The rollback is checked the same way, and the status says so when it was not enough.
#
# An update of the launcher restarts the NAS. This script writes and enables
# nas-os-update-verify.service before it installs anything, and that unit repeats the check after
# the restart. So the NAS is put back also when it breaks only at boot, and when the update was cut
# short by a timeout or a power loss. The unit and its copy of this script go away once the NAS is
# checked.
#
#   install-launcher-update.sh            # the update, run by nas-os-update.service
#   install-launcher-update.sh --verify   # the check after the restart, run by the verify unit
#
# The persisted result is reported by the launcher after startup. Nothing here touches the
# console, SSH, the network configuration of Debian or the units of Debian.

set -uo pipefail
shopt -s nullglob

readonly ROOT=${NAS_UPDATE_ROOT:-}
readonly STATE=${NAS_STATE_DIR:-$ROOT/var/lib/nas-os}
readonly STATUS=$STATE/launcher-update.status
readonly BACKUPS=${NAS_BACKUP_DIR:-$STATE/backups}
readonly PENDING=$STATE/update-verify
readonly STAGING=$STATE/update-staging
readonly VERIFY_UNIT=nas-os-update-verify.service
readonly VERIFY_UNIT_FILE=$ROOT/etc/systemd/system/$VERIFY_UNIT
readonly KEEP_BACKUPS=3
readonly MAX_ATTEMPTS=3
readonly SERVICE=nas-os
readonly GATEWAY=nas-os-gateway
readonly HEALTH_PORT=${NAS_HEALTH_PORT:-3000}
readonly GATEWAY_HEALTH_PORT=${NAS_GATEWAY_HEALTH_PORT:-80}
readonly CONNECTION=$ROOT/var/lib/private/nas-os-gateway/gateway.connection
readonly HEALTH_TRIES=${NAS_HEALTH_TRIES:-30}
readonly BOOT_TRIES=${NAS_BOOT_TRIES:-45}
readonly PAUSE=${NAS_HEALTH_PAUSE:-2}
readonly REBOOT=${NAS_UPDATE_REBOOT:-yes}
readonly SPARE_KB=102400
readonly NET=${NAS_NET_DIR:-/sys/class/net}
read -r -a PROBE <<<"${NAS_UPDATE_PROBE-api.github.com 443}"
readonly PROBE
# Installs the wizard before affected services and updates the launcher last.
readonly COMPONENTS=(wizard gateway launcher)
SELF=$(readlink -f -- "${BASH_SOURCE[0]}")
readonly SELF

# Every path an installer of the wizard, the gateway or the launcher writes, relative to the root.
# A pattern matches only what belongs to the NAS, never a file of Debian. The verify unit is left
# out, it has to outlive the rollback it runs.
readonly FILES=(
  usr/local/bin/nas-os-axum usr/local/bin/nas-os-gateway
  usr/local/share/nas-os usr/local/lib/nas-os usr/local/lib/nas-os-gateway
  usr/local/lib/nas-wizard usr/local/lib/nas-os-configuration
  etc/nas-os var/lib/nas-os-configuration var/lib/nas-wizard
  'etc/systemd/system/nas-*' 'etc/systemd/system/*.wants/nas-*'
  'etc/systemd/system/*.requires/nas-*'
  etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf
  'etc/polkit-1/rules.d/*nas-os*' 'etc/udev/rules.d/*nas-os*' 'etc/pam.d/nas-os*'
)
readonly LAUNCHER_STATE=(var/lib/nas-os/launcher.db var/lib/nas-os/launcher.db-wal
  var/lib/nas-os/launcher.db-shm)
readonly GATEWAY_STATE=(var/lib/private/nas-os-gateway)

backup=''
planned=()
baseline=''
attempts=0
phase=''
reason=''

status() {
  printf '%s\n' "$*" >"$STATUS.new" && mv -f -- "$STATUS.new" "$STATUS"
  sync
}

refuse() {
  status "failed $1"
  printf 'NAS not updated: %s\n' "$1" >&2
  exit 1
}

is_planned() {
  local component
  for component in "${planned[@]}"; do
    [[ $component == "$1" ]] && return 0
  done
  return 1
}

tls_port() {
  local port=${NAS_GATEWAY_TLS_PORT:-}
  if [[ -z $port && -r $CONNECTION ]]; then
    IFS= read -r port <"$CONNECTION"
  fi
  [[ $port =~ ^[1-9][0-9]{0,4}$ ]] || port=443
  printf '%s' "$port"
}

# The patterns the rollback restores, the gateway database only when its service was stopped.
restorable() {
  printf '%s\n' "${FILES[@]}" "${LAUNCHER_STATE[@]}"
  if is_planned gateway; then printf '%s\n' "${GATEWAY_STATE[@]}"; fi
}

# Prints the paths that exist now under the patterns, relative to the root, one per line. A path
# outside the folders of the NAS is never printed, whatever a pattern turns into.
expand() {
  local pattern path
  while IFS= read -r pattern; do
    for path in "${ROOT:-}/"$pattern; do
      [[ -e $path || -L $path ]] || continue
      path=${path#"${ROOT:-}/"}
      [[ -n $path && $path != *..* && $path != *nas-os-update-verify* ]] || continue
      case $path in
        usr/local/bin/nas-* | usr/local/share/nas-os | usr/local/lib/nas-* | etc/nas-os | \
          etc/systemd/system/*nas-* | etc/systemd/system/clamav-freshclam.service.d/nas-os-* | \
          etc/polkit-1/rules.d/*nas-os* | etc/udev/rules.d/*nas-os* | etc/pam.d/nas-os* | \
          var/lib/nas-os-configuration | var/lib/nas-wizard | var/lib/nas-os/launcher.db* | \
          var/lib/private/nas-os-gateway)
          printf '%s\n' "$path"
          ;;
      esac
    done
  done < <(restorable)
}

answers() {
  timeout 5 bash -c 'exec 3<>"/dev/tcp/127.0.0.1/$1" || exit 1
    printf "GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n" >&3
    head -n 1 <&3 | grep -q " 200 "' _ "$1" 2>/dev/null
}

listens() {
  timeout 5 bash -c 'exec 3<>"/dev/tcp/$1/$2"' _ "$1" "$2" 2>/dev/null
}

# The way out of the NAS, tried as the launcher, which the output firewall filters by user.
reaches() {
  ((${#PROBE[@]} == 2)) || return 1
  if ((EUID == 0)) && id -u "$SERVICE" >/dev/null 2>&1; then
    timeout 8 runuser -u "$SERVICE" -- bash -c 'exec 3<>"/dev/tcp/$1/$2"' _ "${PROBE[@]}" \
      2>/dev/null
  else
    listens "${PROBE[@]}"
  fi
}

# Prints one line for each part of the NAS that works now, sorted.
working() {
  local port
  port=$(tls_port)
  {
    answers "$HEALTH_PORT" && echo launcher
    answers "$GATEWAY_HEALTH_PORT" && echo gateway-http
    listens 127.0.0.1 "$port" && echo gateway-https
    if command -v ip >/dev/null; then
      # Only the network cards, a bridge or a tunnel of an application comes and goes.
      ip -4 -o address show scope global 2>/dev/null | awk '{ print $2 }' |
        while IFS= read -r card; do
          [[ $card =~ ^[A-Za-z0-9_.-]+$ && -e $NET/$card/device ]] && echo "address $card"
        done
      [[ -n $(ip -4 route show default 2>/dev/null) ]] && echo route
    fi
    if command -v nft >/dev/null &&
      nft list set inet nas_base nas_https 2>/dev/null | grep -qw -- "$port"; then
      echo firewall
    fi
    reaches && echo internet
  } | LC_ALL=C sort -u
}

describe() {
  case $1 in
    launcher) echo 'the launcher does not answer' ;;
    gateway-http) echo "the gateway does not answer on port $GATEWAY_HEALTH_PORT" ;;
    gateway-https) echo "the gateway does not answer on the HTTPS port $(tls_port)" ;;
    'address '*) echo "the interface ${1#address } lost its address" ;;
    route) echo 'the default route is gone' ;;
    firewall) echo 'the base firewall is not loaded' ;;
    internet) echo "the NAS no longer reaches ${PROBE[0]:-the internet}" ;;
    *) echo "$1 no longer works" ;;
  esac
}

# Says what worked before the update and does not work now, or nothing when all of it works.
lost() {
  local item text=''
  while IFS= read -r item; do
    [[ -n $item ]] && text+="${text:+, }$(describe "$item")"
  done < <(LC_ALL=C comm -23 <(printf '%s\n' "$baseline" | LC_ALL=C sort -u) <(working))
  printf '%s' "$text"
}

wait_for() {
  local try
  for ((try = 0; try < HEALTH_TRIES; try++)); do
    answers "$1" && return 0
    sleep "$PAUSE"
  done
  return 1
}

# Waits, at most the given number of tries, until all that worked before works again.
missing=''
recovered() {
  local try
  for ((try = 0; try < $1; try++)); do
    missing=$(lost)
    [[ -z $missing ]] && return 0
    sleep "$PAUSE"
  done
  return 1
}

mark() {
  {
    printf 'backup=%s\n' "$backup"
    printf 'planned=%s\n' "${planned[*]}"
    printf 'phase=%s\n' "$1"
    printf 'attempts=%s\n' "$attempts"
    printf 'reason=%s\n' "${2:-}"
  } >"$PENDING/pending.new" && mv -f -- "$PENDING/pending.new" "$PENDING/pending"
  sync
}

load() {
  local key value
  while IFS='=' read -r key value; do
    case $key in
      backup) backup=$value ;;
      planned) read -r -a planned <<<"$value" ;;
      phase) phase=$value ;;
      attempts) attempts=$value ;;
      reason) reason=$value ;;
    esac
  done <"$PENDING/pending"
  [[ $attempts =~ ^[0-9]+$ ]] || attempts=$MAX_ATTEMPTS
  baseline=$(cat -- "$PENDING/working" 2>/dev/null)
}

# Writes the check that runs after the restart, before any installer runs.
arm() {
  install -d -m 0700 -- "$PENDING" &&
    install -m 0700 -- "$SELF" "$PENDING/verify.sh" &&
    printf '%s\n' "$baseline" >"$PENDING/working" &&
    mark installing &&
    mkdir -p -- "$(dirname -- "$VERIFY_UNIT_FILE")" || return 1
  cat >"$VERIFY_UNIT_FILE" <<UNIT || return 1
# Written by install-launcher-update.sh for one update, removed once the NAS is checked.
# Turned off from a console with
#   systemctl disable $VERIFY_UNIT && rm -rf $PENDING

[Unit]
Description=nas-os: check the NAS after its update, put the previous version back if it broke
After=network-online.target nas-os.service nas-os-gateway.service nas-os-base-firewall.service
Wants=network-online.target
ConditionPathExists=$PENDING/pending

[Service]
# Started at once, so the boot and multi-user.target never wait for the check.
Type=exec
ExecStart=/bin/bash $PENDING/verify.sh --verify
RuntimeMaxSec=30min
TimeoutStopSec=5min

[Install]
WantedBy=multi-user.target
UNIT
  chmod 0644 -- "$VERIFY_UNIT_FILE" && systemctl daemon-reload && systemctl enable "$VERIFY_UNIT"
}

disarm() {
  systemctl disable "$VERIFY_UNIT" >/dev/null 2>&1
  rm -f -- "$VERIFY_UNIT_FILE"
  rm -rf -- "$PENDING"
  systemctl daemon-reload
}

prune() {
  local kept=("$BACKUPS"/*.tar.gz)
  if ((${#kept[@]} > KEEP_BACKUPS)); then
    rm -f -- "${kept[@]:0:${#kept[@]}-KEEP_BACKUPS}"
  fi
}

# Puts the backup back. The archive is read in full first, so a damaged one changes nothing. The
# units the new version added are stopped, every path the update may have written is removed, so
# what the new version added goes away too, then the archive is extracted over the root.
restore() {
  local members path unit
  [[ -f $backup ]] || return 1
  members=$(tar --list --gzip --file "$backup") || return 1
  while IFS= read -r path; do
    unit=${path#etc/systemd/system/}
    [[ $unit != "$path" && $unit != */* && $unit != *@.* ]] || continue
    [[ $unit == *.service || $unit == *.socket || $unit == *.timer ]] || continue
    grep -qxF -- "$path" <<<"$members" || systemctl stop "$unit"
  done < <(expand)
  while IFS= read -r path; do
    rm -rf -- "${ROOT:-}/$path" || return 1
  done < <(expand)
  tar --extract --gzip --preserve-permissions --numeric-owner --file "$backup" \
    --directory "${ROOT:-/}" || return 1
  enabled=$(grep -E '^etc/systemd/system/(sysinit|multi-user|sockets|timers)\.target\.wants/nas-[^/]+$' \
    <<<"$members" | sed 's|.*/||')
}

# The units of the NAS the previous version had enabled, which the new one may have removed.
enabled=''
start_enabled() {
  local unit
  while IFS= read -r unit; do
    [[ -n $unit && $unit != "$SERVICE.service" && $unit != "$GATEWAY.service" ]] || continue
    systemctl is-active --quiet "$unit" || systemctl start --no-block "$unit"
  done <<<"$enabled"
}

# Loads again what the previous files describe, the firewall above all, which the kernel keeps
# from the new version until it is replaced.
reapply() {
  local loader=$ROOT/usr/local/lib/nas-os-configuration/base-firewall.sh
  systemctl daemon-reload
  if command -v udevadm >/dev/null; then udevadm control --reload; fi
  systemctl try-restart polkit
  if [[ -x $loader ]] && systemctl is-active --quiet nas-os-base-firewall.service; then
    "$loader" || printf 'the previous base firewall could not be loaded again\n' >&2
  fi
  systemctl try-restart nas-os-wifi.service nas-os-screen.service
}

shutting_down() {
  [[ $(systemctl is-system-running 2>/dev/null) == stopping ]]
}

roll_back() {
  trap '' TERM INT HUP
  local why=$1 result=rolled-back
  printf 'NAS update failed, rolling back: %s\n' "$why" >&2
  status "rolling-back $why"
  if shutting_down; then
    mark rolling-back "$why"
    systemctl stop "$SERVICE"
    if is_planned gateway; then systemctl stop "$GATEWAY"; fi
    restore || printf 'the backup %s could not be restored before the shutdown\n' "$backup" >&2
    printf 'the NAS is shutting down, the rollback ends at the next start\n' >&2
    exit 1
  fi
  attempts=$((attempts + 1))
  mark rolling-back "$why"
  systemctl stop "$SERVICE"
  if is_planned gateway; then systemctl stop "$GATEWAY"; fi
  if ! restore; then
    result=rollback-failed
    why="$why, and the backup $backup could not be restored"
  fi
  reapply
  start_enabled
  systemctl start "$SERVICE"
  if is_planned gateway; then
    systemctl start "$GATEWAY"
  else
    systemctl try-restart "$GATEWAY"
  fi
  if [[ $result == rolled-back ]] && ! recovered "$HEALTH_TRIES"; then
    result=rollback-failed
    why="$why, and the previous version does not work either ($missing)"
  fi
  status "$result $why"
  disarm
  printf 'NAS %s: %s\n' "$result" "$why" >&2
  exit 1
}

finish() {
  status "done ${planned[*]}"
  disarm
  prune
  printf 'NAS updated (%s), backup in %s\n' "${planned[*]}" "$backup"
}

verify() {
  [[ -f $PENDING/pending ]] || {
    disarm
    exit 0
  }
  load
  if ((attempts >= MAX_ATTEMPTS)); then
    status "rollback-failed ${reason:-the previous version could not be put back}"
    disarm
    # Whatever is on the disk now runs, a rollback cut short may have left the services stopped.
    systemctl start "$SERVICE" "$GATEWAY"
    exit 1
  fi
  case $phase in
    installing) roll_back 'the update was cut short before it ended' ;;
    rolling-back) roll_back "${reason:-the update was cut short while it was put back}" ;;
    installed)
      status "verifying ${planned[*]}"
      if recovered "$BOOT_TRIES"; then
        finish
        exit 0
      fi
      roll_back "after the restart $missing"
      ;;
    *)
      status 'rollback-failed the state of the update cannot be read'
      disarm
      exit 1
      ;;
  esac
}

case ${1:-} in
  --verify) mode=verify ;;
  '') mode=update ;;
  *)
    printf 'usage: %s [--verify]\n' "$0" >&2
    exit 2
    ;;
esac

exec 9>>"$STATE/update.lock" || exit 1
if ! flock -n 9; then
  printf 'another update of the NAS is running\n' >&2
  exit 1
fi

if [[ $mode == verify ]]; then
  verify
fi

for component in "${COMPONENTS[@]}"; do
  [[ -r $STATE/$component-update.tar ]] && planned+=("$component")
done
((${#planned[@]} > 0)) || refuse "no release in $STATE"
trap 'rm -rf -- "$STAGING"; for c in "${COMPONENTS[@]}"; do rm -f -- "$STATE/$c-update.tar"; done' EXIT
[[ ! -e $PENDING/pending ]] || refuse 'the previous update has not been checked yet'
command -v tar >/dev/null || refuse 'tar is not installed'

declare -A installer=(
  [wizard]=install.sh
  [gateway]=packages/gateway_installation/install.sh
  [launcher]=packages/ui_installation/install.sh
)
rm -rf -- "$STAGING"
install -d -m 0700 -- "$STAGING" || refuse 'no folder to unpack the releases'
for component in "${planned[@]}"; do
  mkdir -p -- "$STAGING/$component"
  tar --extract --auto-compress --no-same-owner --file "$STATE/$component-update.tar" \
    --directory "$STAGING/$component" || refuse "the release of the $component is not a readable archive"
  [[ -f $STAGING/$component/${installer[$component]} ]] ||
    refuse "the release of the $component has no installer"
done

baseline=$(working)
printf 'working before the update: %s\n' "$(tr '\n' ' ' <<<"$baseline")"

status 'backing-up'
mkdir -p -m 0700 -- "$BACKUPS" || refuse "cannot create $BACKUPS"
mapfile -t saved < <(expand)
((${#saved[@]} > 0)) || refuse 'nothing of the NAS to back up'
need=$(cd -- "${ROOT:-/}" && du -skc -- "${saved[@]}" 2>/dev/null | tail -n 1 | cut -f 1)
free=$(df -Pk -- "$BACKUPS" | awk 'NR == 2 { print $4 }')
[[ $need =~ ^[0-9]+$ && $free =~ ^[0-9]+$ ]] && ((free > need + SPARE_KB)) ||
  refuse "not enough free space in $BACKUPS for the backup"

abandon() {
  trap '' TERM INT HUP
  rm -f -- "$backup"
  disarm
  systemctl start "$SERVICE"
  if is_planned gateway; then systemctl start "$GATEWAY"; fi
  refuse "$1"
}

backup=$BACKUPS/$(date +%Y%m%d-%H%M%S).tar.gz
trap 'abandon "the update was stopped before it began"' TERM INT HUP
systemctl stop "$SERVICE" || refuse "$SERVICE did not stop"
if is_planned gateway; then
  systemctl stop "$GATEWAY" || {
    systemctl start "$SERVICE"
    refuse "$GATEWAY did not stop"
  }
fi

mapfile -t saved < <(expand)
if ! tar --create --gzip --numeric-owner --file "$backup" --directory "${ROOT:-/}" -- "${saved[@]}" ||
  ! tar --list --gzip --file "$backup" >/dev/null; then
  abandon 'the backup could not be written'
fi
chmod 0600 -- "$backup"
arm || abandon 'the check after the update could not be prepared'

trap 'roll_back "the update was stopped before it ended"' TERM INT HUP

if is_planned wizard; then
  status 'installing-wizard'
  bash "$STAGING/wizard/${installer[wizard]}" --update || roll_back 'the installer of the wizard failed'
fi

if is_planned gateway; then
  status 'installing-gateway'
  # The installer starts the gateway it put in place.
  bash "$STAGING/gateway/${installer[gateway]}" || roll_back 'the installer of the gateway failed'
  if grep -qx gateway-http <<<"$baseline"; then
    wait_for "$GATEWAY_HEALTH_PORT" || roll_back 'the new gateway does not answer'
  fi
fi

if is_planned launcher; then
  status 'installing-launcher'
  bash "$STAGING/launcher/${installer[launcher]}" || roll_back 'the installer of the launcher failed'
else
  systemctl start "$SERVICE"
fi

status 'checking'
recovered "$HEALTH_TRIES" || roll_back "$missing"

if is_planned launcher && [[ $REBOOT == yes ]]; then
  trap - TERM INT HUP
  mark installed
  status "restarting ${planned[*]}"
  printf 'NAS updated (%s), checked again after the restart\n' "${planned[*]}"
  systemctl reboot && exit 0
  printf 'the NAS did not restart, the update is checked now\n' >&2
fi
trap - TERM INT HUP
finish
