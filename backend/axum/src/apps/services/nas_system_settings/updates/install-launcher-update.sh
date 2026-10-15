#!/usr/bin/env bash
#
# Installs the releases of the NAS the launcher downloaded, together: the wizard, the gateway and
# the launcher, in that order, whichever of them have an update. Then it reboots the machine when
# the launcher changed, and only starts the launcher again otherwise.
#
# It runs as root from `nas-os-update.service`, which the launcher starts once it has fetched every
# release and checked it against its published checksum: a running service cannot replace itself.
# Each release waits in the state directory as `<component>-update.tar`, and its own installer puts
# it in place: `install.sh --update` for the wizard, `gateway_installation/install.sh` for the
# gateway, `ui_installation/install.sh` for the launcher.
#
# Before anything changes, the launcher (and the gateway, when it is updated) is stopped, so the
# databases on disk are consistent, and the databases, the configuration and every installed file
# are saved in one backup. If any installer fails, or the gateway or the new launcher does not
# answer, the backup is put back and the previous NAS is started again: the update is all or
# nothing. How it ended is written next to the database, and the launcher reports it at its next
# start.

set -uo pipefail

readonly STATE=${NAS_STATE_DIR:-/var/lib/nas-os}
readonly STATUS=$STATE/launcher-update.status
readonly BACKUPS=${NAS_BACKUP_DIR:-$STATE/backups}
readonly KEEP_BACKUPS=3
readonly SERVICE=nas-os
readonly GATEWAY=nas-os-gateway
readonly HEALTH_PORT=${NAS_HEALTH_PORT:-3000}
readonly GATEWAY_HEALTH_PORT=${NAS_GATEWAY_HEALTH_PORT:-80}
readonly HEALTH_TRIES=${NAS_HEALTH_TRIES:-30}
readonly REBOOT=${NAS_UPDATE_REBOOT:-yes}
# The order of installation: the wizard changes nothing that runs, the launcher restarts last.
readonly COMPONENTS=(wizard gateway launcher)

read -r -a SAVED <<<"${NAS_UPDATE_PATHS:-$STATE/launcher.db $STATE/launcher.db-wal $STATE/launcher.db-shm \
/etc/nas-os /usr/local/bin/nas-os-axum /usr/local/share/nas-os \
/usr/local/lib/nas-os /etc/systemd/system/nas-os.service \
/etc/systemd/system/nas-os-wake.service \
/etc/systemd/system/nas-os-firewall.service /etc/systemd/system/nas-os-update.service \
/etc/systemd/system/nas-app@.service /etc/systemd/system/nas-os-apt@.service \
/etc/polkit-1/rules.d/10-nas-os-power.rules /etc/polkit-1/rules.d/20-nas-os-network.rules \
/etc/polkit-1/rules.d/30-nas-os-firewall.rules /etc/polkit-1/rules.d/40-nas-os-update.rules \
/etc/polkit-1/rules.d/50-nas-os-apps.rules /etc/polkit-1/rules.d/60-nas-os-apt.rules}"
read -r -a SAVED_GATEWAY <<<"${NAS_UPDATE_GATEWAY_PATHS:-/usr/local/bin/nas-os-gateway \
/etc/systemd/system/nas-os-gateway.service /usr/local/lib/nas-os-gateway \
/var/lib/private/nas-os-gateway/gateway.db /var/lib/private/nas-os-gateway/gateway.db-wal \
/var/lib/private/nas-os-gateway/gateway.db-shm /var/lib/private/nas-os-gateway/gateway.connection}"
read -r -a SAVED_WIZARD <<<"${NAS_UPDATE_WIZARD_PATHS:-/usr/local/lib/nas-wizard \
/etc/systemd/system/nas-wizard.service}"

backup=''
planned=()

status() {
  printf '%s\n' "$*" >"$STATUS"
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

roll_back() {
  printf 'NAS update failed, rolling back: %s\n' "$1" >&2
  systemctl stop "$SERVICE"
  is_planned gateway && systemctl stop "$GATEWAY"
  rm -f "$STATE/launcher.db-wal" "$STATE/launcher.db-shm"
  tar --extract --gzip --preserve-permissions --file "$backup" --directory / ||
    printf 'the backup %s could not be restored completely\n' "$backup" >&2
  systemctl daemon-reload
  systemctl try-restart polkit
  systemctl start "$SERVICE"
  if is_planned gateway; then
    systemctl start "$GATEWAY"
  else
    systemctl try-restart "$GATEWAY"
  fi
  status "rolled-back $1"
  exit 1
}

answers() {
  (
    exec 3<>"/dev/tcp/127.0.0.1/$1" || exit 1
    printf 'GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3
    head -n 1 <&3 | grep -q ' 200 '
  ) 2>/dev/null
}

wait_for() {
  local try
  for ((try = 0; try < HEALTH_TRIES; try++)); do
    answers "$1" && return 0
    sleep 2
  done
  return 1
}

for component in "${COMPONENTS[@]}"; do
  [[ -r $STATE/$component-update.tar ]] && planned+=("$component")
done
((${#planned[@]} > 0)) || refuse "no release in $STATE"
command -v tar >/dev/null || refuse 'tar is not installed'

staging=$(mktemp -d) || refuse 'no temporary directory'
trap 'rm -rf "$staging"; for c in "${COMPONENTS[@]}"; do rm -f "$STATE/$c-update.tar"; done' EXIT

declare -A installer=(
  [wizard]=install.sh
  [gateway]=packages/gateway_installation/install.sh
  [launcher]=packages/ui_installation/install.sh
)
for component in "${planned[@]}"; do
  mkdir -p "$staging/$component"
  tar --extract --auto-compress --no-same-owner --file "$STATE/$component-update.tar" \
    --directory "$staging/$component" || refuse "the release of the $component is not a readable archive"
  [[ -f $staging/$component/${installer[$component]} ]] ||
    refuse "the release of the $component has no installer"
done

status 'backing-up'
mkdir -p "$BACKUPS" || refuse "cannot create $BACKUPS"
backup=$BACKUPS/$(date +%Y%m%d-%H%M%S).tar.gz
systemctl stop "$SERVICE" || refuse "$SERVICE did not stop"
saved=("${SAVED[@]}")
if is_planned gateway; then
  systemctl stop "$GATEWAY" || {
    systemctl start "$SERVICE"
    refuse "$GATEWAY did not stop"
  }
  saved+=("${SAVED_GATEWAY[@]}")
fi
is_planned wizard && saved+=("${SAVED_WIZARD[@]}")
existing=()
for path in "${saved[@]}"; do
  [[ -e $path ]] && existing+=("${path#/}")
done
if ! tar --create --gzip --file "$backup" --directory / -- "${existing[@]}"; then
  rm -f "$backup"
  systemctl start "$SERVICE"
  is_planned gateway && systemctl start "$GATEWAY"
  refuse 'the backup could not be written'
fi
chmod 0600 "$backup"

if is_planned wizard; then
  status 'installing-wizard'
  bash "$staging/wizard/${installer[wizard]}" --update || roll_back 'the installer of the wizard failed'
fi

if is_planned gateway; then
  status 'installing-gateway'
  # The installer starts the gateway it put in place.
  bash "$staging/gateway/${installer[gateway]}" || roll_back 'the installer of the gateway failed'
  wait_for "$GATEWAY_HEALTH_PORT" || roll_back 'the new gateway does not answer'
fi

if is_planned launcher; then
  status 'installing-launcher'
  bash "$staging/launcher/${installer[launcher]}" || roll_back 'the installer of the launcher failed'
else
  systemctl start "$SERVICE"
fi
wait_for "$HEALTH_PORT" || roll_back 'the launcher does not answer'

kept=("$BACKUPS"/*.tar.gz)
if ((${#kept[@]} > KEEP_BACKUPS)); then
  rm -f -- "${kept[@]:0:${#kept[@]}-KEEP_BACKUPS}"
fi

status "done ${planned[*]}"
printf 'NAS updated (%s); backup in %s\n' "${planned[*]}" "$backup"
if is_planned launcher && [[ $REBOOT == yes ]]; then
  systemctl reboot
fi
