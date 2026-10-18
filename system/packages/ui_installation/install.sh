#!/usr/bin/env bash
#
# Installs the launcher on the NAS.
# It can be run again at any time: it repeats only what is missing or changed, and it doubles as
# the update step after a new build.
#
#   sudo ./install.sh [path/to/binary]
#   ./install.sh --dry-run        # prints the commands without running them
#
# It runs from the release package (`npm run release:package`), which puts the binary in bin/, the
# interface in ui/ and the backend helpers in helpers/ next to this script. The wizard runs it
# after the configuration of the operating system and the installation of the gateway, which are
# not part of this package. The update of the launcher runs it again.

set -euo pipefail

readonly SERVICE_USER=nas-os
readonly SERVICE_NAME=nas-os
readonly BINARY_TARGET=/usr/local/bin/nas-os-axum
readonly UNIT_TARGET=/etc/systemd/system/nas-os.service
readonly POLKIT_TARGET=/etc/polkit-1/rules.d/10-nas-os-power.rules
readonly POLKIT_NETWORK_TARGET=/etc/polkit-1/rules.d/20-nas-os-network.rules
readonly POLKIT_FIREWALL_TARGET=/etc/polkit-1/rules.d/30-nas-os-firewall.rules
readonly POLKIT_UPDATE_TARGET=/etc/polkit-1/rules.d/40-nas-os-update.rules
readonly POLKIT_APPS_TARGET=/etc/polkit-1/rules.d/50-nas-os-apps.rules
readonly POLKIT_GRANTS_TARGET=/etc/polkit-1/rules.d/55-nas-os-app-grants.rules
readonly POLKIT_APP_PACKAGES_TARGET=/etc/polkit-1/rules.d/56-nas-os-app-packages.rules
readonly POLKIT_SHARES_TARGET=/etc/polkit-1/rules.d/58-nas-os-smb-shares.rules
readonly POLKIT_APT_TARGET=/etc/polkit-1/rules.d/60-nas-os-apt.rules
readonly POLKIT_PROCESSES_TARGET=/etc/polkit-1/rules.d/65-nas-os-processes.rules
readonly POLKIT_TOKEN_TARGET=/etc/polkit-1/rules.d/70-nas-os-token.rules
readonly POLKIT_STORAGE_TARGET=/etc/polkit-1/rules.d/90-nas-os-storage.rules
readonly POLKIT_HOSTNAME_TARGET=/etc/polkit-1/rules.d/95-nas-os-hostname.rules
readonly POLKIT_RESET_TARGET=/etc/polkit-1/rules.d/75-nas-os-reset.rules
readonly WAKE_UNIT_TARGET=/etc/systemd/system/nas-os-wake.service
readonly WAKE_SCRIPT_TARGET=/usr/local/lib/nas-os/wake-alarm.sh
readonly FIREWALL_UNIT_TARGET=/etc/systemd/system/nas-os-firewall.service
readonly FIREWALL_SCRIPT_TARGET=/usr/local/lib/nas-os/apply-access-rules.sh
readonly UPDATE_UNIT_TARGET=/etc/systemd/system/nas-os-update.service
readonly UPDATE_SCRIPT_TARGET=/usr/local/lib/nas-os/install-launcher-update.sh
readonly TOKEN_UNIT_TARGET=/etc/systemd/system/nas-os-token.service
readonly TOKEN_SCRIPT_TARGET=/usr/local/lib/nas-os/launcher-token.sh
readonly APP_UNIT_TARGET='/etc/systemd/system/nas-app@.service'
readonly GRANTS_UNIT_TARGET='/etc/systemd/system/nas-os-app-grants@.service'
readonly GRANTS_SCRIPT_TARGET=/usr/local/lib/nas-os/apply-app-grants.sh
readonly APP_PACKAGES_UNIT_TARGET='/etc/systemd/system/nas-os-app-packages@.service'
readonly APP_PACKAGES_SCRIPT_TARGET=/usr/local/lib/nas-os/app-packages.sh
readonly SHARES_UNIT_TARGET=/etc/systemd/system/nas-os-smb-shares.service
readonly SHARES_SCRIPT_TARGET=/usr/local/lib/nas-os/apply-smb-shares.sh
readonly PROCESS_UNIT_TARGET='/etc/systemd/system/nas-os-process@.service'
readonly PROCESS_SCRIPT_TARGET=/usr/local/lib/nas-os/act-on-process.sh
readonly STORAGE_UNIT_TARGET=/etc/systemd/system/nas-os-storage.service
readonly STORAGE_INITIALIZE_UNIT_TARGET=/etc/systemd/system/nas-os-storage-initialize.service
readonly STORAGE_RENAME_UNIT_TARGET=/etc/systemd/system/nas-os-storage-rename.service
readonly STORAGE_EJECT_UNIT_TARGET=/etc/systemd/system/nas-os-storage-eject.service
readonly STORAGE_SMART_UNIT_TARGET=/etc/systemd/system/nas-os-storage-smart.service
readonly STORAGE_TIMER_TARGET=/etc/systemd/system/nas-os-storage.timer
readonly STORAGE_CRYPT_SOCKET_TARGET=/etc/systemd/system/nas-os-storage-crypt.socket
readonly STORAGE_CRYPT_UNIT_TARGET='/etc/systemd/system/nas-os-storage-crypt@.service'
readonly STORAGE_UDEV_TARGET=/etc/udev/rules.d/90-nas-os-storage.rules
readonly HOSTNAME_UNIT_TARGET=/etc/systemd/system/nas-os-hostname.service
readonly HOSTNAME_SCRIPT_TARGET=/usr/local/lib/nas-os/apply-hostname.sh
readonly RESET_UNIT_TARGET=/etc/systemd/system/nas-os-reset.service
readonly RESET_SCRIPT_TARGET=/usr/local/lib/nas-os/factory-reset.sh
readonly APP_SCRIPT_TARGET=/usr/local/lib/nas-os/run-app.sh
readonly APT_UNIT_TARGET='/etc/systemd/system/nas-os-apt@.service'
readonly APT_SCRIPT_TARGET=/usr/local/lib/nas-os/apt-updates.sh
readonly TERMINAL_USER_SCRIPT_TARGET=/usr/local/lib/nas-os/terminal-user.sh
readonly TERMINAL_SOCKET_TARGET=/etc/systemd/system/nas-os-terminal.socket
readonly TERMINAL_UNIT_TARGET='/etc/systemd/system/nas-os-terminal@.service'
readonly UI_TARGET=/usr/local/share/nas-os/ui
readonly CONFIG_DIR=/etc/nas-os
readonly CATALOGUE_TARGET=/etc/nas-os/catalogue.json
readonly SETTINGS_TARGET=/etc/nas-os/nas-os.env
readonly APPS_DIR=/var/lib/nas-os/apps
readonly LOGOS_TARGET=/usr/local/share/nas-os/logos
readonly SOCKETS_DIR=/run/nas-os/apps
readonly STATE_DIR=/var/lib/nas-os
readonly TOOLS_DIR=/usr/local/lib/nas-os

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
dry_run=false
binary_source=""

for argument in "$@"; do
  case $argument in
    --dry-run) dry_run=true ;;
    -*) printf 'unknown option: %s\n' "$argument" >&2 && exit 2 ;;
    *) binary_source=$argument ;;
  esac
done
binary_source=${binary_source:-"$here/bin/nas-os-axum"}

say() { printf '\n== %s\n' "$1"; }
fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}
run() {
  if $dry_run; then
    printf '   [dry run] %s\n' "$*"
  else
    "$@"
  fi
}

if ! $dry_run; then
  [[ $EUID -eq 0 ]] || fail "it has to run as root: sudo $0"
fi
[[ -f $binary_source ]] || fail "binary not found at $binary_source
  build it first:  cargo build --release --manifest-path backend/Cargo.toml
  or run this script from the release package (npm run release:package)"
command -v systemctl >/dev/null || fail "systemd is not available on this machine"
$dry_run && printf '\nDry run: no change will be applied.\n'

say "Required system packages"
if command -v parted >/dev/null && command -v mkfs.ext4 >/dev/null; then
  echo "   parted and e2fsprogs present"
elif $dry_run; then
  echo "   parted and e2fsprogs missing: the wizard installs them before this script"
else
  fail "parted and e2fsprogs are required: run the wizard, which configures the system first"
fi

run install -d -m 0755 -o root -g root "$TOOLS_DIR"
run install -m 0755 -o root -g root "$here/scripts/uninstall.sh" "$TOOLS_DIR/uninstall.sh"

say "Service user"
if id -u "$SERVICE_USER" >/dev/null 2>&1; then
  echo "   $SERVICE_USER already exists"
else
  run useradd --system --home-dir "$STATE_DIR" --shell /usr/sbin/nologin "$SERVICE_USER"
  echo "   created $SERVICE_USER"
fi
if getent group nas-storage >/dev/null; then
  echo "   nas-storage group already exists"
else
  run groupadd --system nas-storage
fi
if getent group nas-os-crypt >/dev/null; then
  echo "   nas-os-crypt group already exists"
else
  run groupadd --system nas-os-crypt
fi
run systemctl try-restart nas-os-base-firewall.service

say "Binary"
run install -m 0755 -o root -g root "$binary_source" "$BINARY_TARGET"
echo "   $binary_source -> $BINARY_TARGET"

say "Interface"
if [[ -d $here/ui ]]; then
  run rm -rf -- "$UI_TARGET.new" "$UI_TARGET.old"
  run install -d -m 0755 -o root -g root "$UI_TARGET.new"
  run cp -rT "$here/ui" "$UI_TARGET.new"
  run chmod -R u=rwX,go=rX "$UI_TARGET.new"
  if [[ -d $UI_TARGET ]]; then
    run mv -T -- "$UI_TARGET" "$UI_TARGET.old"
  fi
  run mv -T -- "$UI_TARGET.new" "$UI_TARGET"
  run rm -rf -- "$UI_TARGET.old"
  echo "   $here/ui -> $UI_TARGET"
else
  echo "   no ui/ next to this script: the interface is left as it is"
fi

say "Power, network, firewall, update, application and package permissions (polkit)"
if [[ -d /etc/polkit-1/rules.d ]]; then
  run install -m 0644 -o root -g root "$here/files/10-nas-os-power.rules" "$POLKIT_TARGET"
  run install -m 0644 -o root -g root "$here/files/20-nas-os-network.rules" \
    "$POLKIT_NETWORK_TARGET"
  run install -m 0644 -o root -g root "$here/files/30-nas-os-firewall.rules" \
    "$POLKIT_FIREWALL_TARGET"
  run install -m 0644 -o root -g root "$here/files/40-nas-os-update.rules" \
    "$POLKIT_UPDATE_TARGET"
  run install -m 0644 -o root -g root "$here/files/50-nas-os-apps.rules" \
    "$POLKIT_APPS_TARGET"
  run install -m 0644 -o root -g root "$here/files/55-nas-os-app-grants.rules" \
    "$POLKIT_GRANTS_TARGET"
  run install -m 0644 -o root -g root "$here/files/56-nas-os-app-packages.rules" \
    "$POLKIT_APP_PACKAGES_TARGET"
  run install -m 0644 -o root -g root "$here/files/57-nas-os-antivirus.rules" /etc/polkit-1/rules.d/57-nas-os-antivirus.rules
  run install -m 0644 -o root -g root "$here/files/58-nas-os-smb-shares.rules" \
    "$POLKIT_SHARES_TARGET"
  run install -m 0644 -o root -g root "$here/files/59-nas-os-cleanup.rules" \
    /etc/polkit-1/rules.d/59-nas-os-cleanup.rules
  run install -m 0644 -o root -g root "$here/files/60-nas-os-apt.rules" "$POLKIT_APT_TARGET"
  run install -m 0644 -o root -g root "$here/files/62-nas-os-sync-ports.rules" \
    /etc/polkit-1/rules.d/62-nas-os-sync-ports.rules
  run install -m 0644 -o root -g root "$here/files/63-nas-os-discovery.rules" \
    /etc/polkit-1/rules.d/63-nas-os-discovery.rules
  run install -m 0644 -o root -g root "$here/files/65-nas-os-processes.rules" \
    "$POLKIT_PROCESSES_TARGET"
  run install -m 0644 -o root -g root "$here/files/70-nas-os-token.rules" "$POLKIT_TOKEN_TARGET"
  run install -m 0644 -o root -g root "$here/files/90-nas-os-storage.rules" \
    "$POLKIT_STORAGE_TARGET"
  run install -m 0644 -o root -g root "$here/files/95-nas-os-hostname.rules" \
    "$POLKIT_HOSTNAME_TARGET"
  run install -m 0644 -o root -g root "$here/files/75-nas-os-reset.rules" "$POLKIT_RESET_TARGET"
  # The wizard creates the user of the terminal now, as root: the launcher no longer asks for it.
  run rm -f /etc/polkit-1/rules.d/80-nas-os-terminal.rules
  run systemctl restart polkit
  echo "   rules installed: power, network, access list, updates, applications, system packages,"
  echo "   processes, storage, hostname, the reset and the release token allowed to $SERVICE_USER"
else
  echo "   polkit is not here (the wizard installs it first): power, network, the access list,"
  echo "   updates, starting an application, system packages, processes, storage, hostname and the"
  echo "   release token will stay refused"
fi

say "NAS hostname"
run install -d -m 0755 -o root -g root "$(dirname "$HOSTNAME_SCRIPT_TARGET")"
run install -m 0755 -o root -g root "$here/helpers/apply-hostname.sh" "$HOSTNAME_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-hostname.service" "$HOSTNAME_UNIT_TARGET"

say "Reset"
run install -m 0755 -o root -g root "$here/helpers/factory-reset.sh" "$RESET_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-reset.service" "$RESET_UNIT_TARGET"

say "Wake alarm"
run install -d -m 0755 -o root -g root "$(dirname "$WAKE_SCRIPT_TARGET")"
run install -m 0755 -o root -g root "$here/helpers/wake-alarm.sh" "$WAKE_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-wake.service" "$WAKE_UNIT_TARGET"
if [[ -e /sys/class/rtc/rtc0/wakealarm ]]; then
  echo "   unit installed: the scheduled power-on will use /sys/class/rtc/rtc0/wakealarm"
else
  echo "   warning: this machine does not expose /sys/class/rtc/rtc0/wakealarm"
  echo "   the scheduled shutdown will work, the power-on will not"
fi

say "Allowed addresses"
run install -m 0755 -o root -g root "$here/helpers/apply-access-rules.sh" "$FIREWALL_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-firewall.service" "$FIREWALL_UNIT_TARGET"
if command -v nft >/dev/null; then
  echo "   unit installed: the saved list will become an nftables set"
else
  echo "   warning: nft is not installed (the wizard installs it first)"
  echo "   the list will hold inside the launcher only, without filtering the rest of the NAS"
fi

say "Application packages"
run install -d -m 0755 -o root -g root "$CONFIG_DIR"
if [[ -f $CATALOGUE_TARGET ]]; then
  echo "   catalogue already there: $CATALOGUE_TARGET (left as it is)"
else
  run install -m 0644 -o root -g root "$here/files/catalogue.json" "$CATALOGUE_TARGET"
  echo "   starter catalogue written to $CATALOGUE_TARGET"
fi
run install -d -m 0755 -o root -g root "$LOGOS_TARGET"
for logo in "$here"/files/logos/*; do
  run install -m 0644 -o root -g root "$logo" "$LOGOS_TARGET/"
done
run install -d -m 0750 -o "$SERVICE_USER" -g "$SERVICE_USER" "$APPS_DIR"
run find "$APPS_DIR" -mindepth 1 -maxdepth 1 -exec chmod -R g=u-w {} +
if [[ -d $here/apps ]]; then
  shopt -s nullglob
  for package in "$here"/apps/*.tar.gz; do
    id=${package##*/}
    id=${id%.tar.gz}
    [[ $id =~ ^[a-zA-Z0-9_-]{1,64}$ ]] || fail "invalid bundled application id: $id"
    if [[ -d $APPS_DIR/$id ]]; then
      echo "   $id already installed: left as it is"
      continue
    fi
    staging="$APPS_DIR/.$id.bundled"
    archive="$APPS_DIR/.$id.bundled.tar.gz"
    run rm -rf -- "$staging"
    run rm -f -- "$archive"
    run install -m 0600 -o "$SERVICE_USER" -g "$SERVICE_USER" "$package" "$archive"
    run install -d -m 0750 -o "$SERVICE_USER" -g "$SERVICE_USER" "$staging"
    run runuser -u "$SERVICE_USER" -- tar -xzf "$archive" --no-same-owner \
      --directory "$staging"
    run rm -f -- "$archive"
    if ! $dry_run; then
      [[ -f $staging/nas-app.json ]] || fail "$package has no nas-app.json at its root"
    fi
    run mv -- "$staging" "$APPS_DIR/$id"
    echo "   bundled $id -> $APPS_DIR/$id"
  done
  shopt -u nullglob
fi
run install -m 0755 -o root -g root "$here/helpers/install-launcher-update.sh" "$UPDATE_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-update.service" "$UPDATE_UNIT_TARGET"
run install -m 0755 -o root -g root "$here/helpers/launcher-token.sh" "$TOKEN_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-token.service" "$TOKEN_UNIT_TARGET"
if command -v tar >/dev/null; then
  echo "   packages will be unpacked into $APPS_DIR"
else
  echo "   warning: tar is not installed, and a package cannot be unpacked without it"
fi

say "Running the applications"
run install -m 0755 -o root -g root "$here/helpers/run-app.sh" "$APP_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-app@.service" "$APP_UNIT_TARGET"
run install -d -m 0700 -o root -g root /srv/nas-volumes
run install -d -m 0710 -o root -g nas-storage /srv/storage
run install -m 0755 -o root -g root "$here/helpers/apply-app-grants.sh" "$GRANTS_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-app-grants@.service" "$GRANTS_UNIT_TARGET"
run install -m 0755 -o root -g root "$here/helpers/app-packages.sh" "$APP_PACKAGES_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-app-packages@.service" \
  "$APP_PACKAGES_UNIT_TARGET"
run install -m 0755 -o root -g root "$here/helpers/apply-smb-shares.sh" "$SHARES_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-smb-shares.service" "$SHARES_UNIT_TARGET"
run rm -rf -- '/etc/systemd/system/nas-app@system-explorer.service.d'
run rm -f -- '/etc/systemd/system/nas-app@nas-file-manager.service.d/storage.conf'
run install -m 0755 -o root -g root "$here/helpers/apply-sync-ports.sh" /usr/local/lib/nas-os/apply-sync-ports.sh
run install -m 0644 -o root -g root "$here/files/nas-os-sync-ports.service" /etc/systemd/system/nas-os-sync-ports.service
run install -m 0755 -o root -g root "$here/helpers/apply-discovery.sh" /usr/local/lib/nas-os/apply-discovery.sh
run install -m 0644 -o root -g root "$here/files/nas-os-discovery.service" /etc/systemd/system/nas-os-discovery.service
run install -m 0644 -o root -g root "$here/files/nas-os-cleanup.service" /etc/systemd/system/nas-os-cleanup.service
run install -m 0644 -o root -g root "$here/files/nas-os-antivirus.service" /etc/systemd/system/nas-os-antivirus.service
run install -m 0644 -o root -g root "$here/files/nas-os-antivirus-lifecycle.service" /etc/systemd/system/nas-os-antivirus-lifecycle.service
run install -d -m 0755 -o root -g root /etc/systemd/system/clamav-freshclam.service.d
run install -m 0644 -o root -g root "$here/files/clamav-freshclam-firewall.conf" \
  /etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf
run install -m 0644 -o root -g root "$here/files/nas-os-storage.service" "$STORAGE_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-initialize.service" \
  "$STORAGE_INITIALIZE_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-rename.service" \
  "$STORAGE_RENAME_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-eject.service" \
  "$STORAGE_EJECT_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-smart.service" \
  "$STORAGE_SMART_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage.timer" "$STORAGE_TIMER_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-crypt.socket" "$STORAGE_CRYPT_SOCKET_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-storage-crypt@.service" "$STORAGE_CRYPT_UNIT_TARGET"
run install -m 0644 -o root -g root "$here/files/90-nas-os-storage-udev.rules" \
  "$STORAGE_UDEV_TARGET"
run udevadm control --reload
echo "   template installed: an application runs as nas-app@<id>.service"

say "System packages"
run install -m 0755 -o root -g root "$here/helpers/apt-updates.sh" "$APT_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-apt@.service" "$APT_UNIT_TARGET"
echo "   template installed: the index and the upgrades run as nas-os-apt@<job>.service"

say "Processes"
run install -m 0755 -o root -g root "$here/helpers/act-on-process.sh" "$PROCESS_SCRIPT_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-process@.service" "$PROCESS_UNIT_TARGET"
echo "   template installed: each request runs as nas-os-process@<action>.service"

say "Terminal"
run install -m 0755 -o root -g root "$here/helpers/terminal-user.sh" "$TERMINAL_USER_SCRIPT_TARGET"
run rm -f /etc/systemd/system/nas-os-terminal-user.service
run install -m 0644 -o root -g root "$here/files/nas-os-terminal.socket" "$TERMINAL_SOCKET_TARGET"
run install -m 0644 -o root -g root "$here/files/nas-os-terminal@.service" "$TERMINAL_UNIT_TARGET"
echo "   units installed: each shell runs as the user of the terminal, never as $SERVICE_USER"

say "Tools on the machine"
run install -m 0755 -o root -g root "$here/scripts/check.sh" "$TOOLS_DIR/check.sh"
run install -m 0755 -o root -g root "$here/../ui_configuration/install.sh" "$TOOLS_DIR/configure.sh"
echo "   $TOOLS_DIR/check.sh, uninstall.sh and configure.sh"

say "Settings"
run install -d -m 0755 -o root -g root "$CONFIG_DIR"
run install -m 0644 -o root -g root "$here/files/nas-os.env" "$SETTINGS_TARGET"
echo "   settings written to $SETTINGS_TARGET (local changes: $CONFIG_DIR/nas-os.local.env)"

say "Systemd unit"
run install -m 0644 -o root -g root "$here/files/nas-os.service" "$UNIT_TARGET"
run systemctl daemon-reload
run systemctl enable "$SERVICE_NAME"
run systemctl restart "$SERVICE_NAME"
run systemctl enable --now nas-os-storage.timer
run systemctl enable --now nas-os-storage-crypt.socket
run systemctl start nas-os-storage.service
echo "   $SERVICE_NAME started and enabled at boot"

say "Done"
cat <<EOF
   interface:  served by the gateway (nas-os-gateway.service), installed with it
   status:     systemctl status $SERVICE_NAME
   log:        journalctl -u $SERVICE_NAME -f
   check:      $TOOLS_DIR/check.sh
   database:   $STATE_DIR/launcher.db  (not touched by this script)
   catalogue:  $CATALOGUE_TARGET      (edit the file to change the offered apps)
   packages:   $APPS_DIR
   sockets:    $SOCKETS_DIR  (created by systemd when the service starts)
   app units:  nas-app@<id>.service  (systemctl status 'nas-app@*')

   An installed application is opened at /apps/<id>/, on the address this machine is reached at:
   nothing to configure on the network.

   The launcher updates itself from NAS_LAUNCHER_REPOSITORY in the unit. For a private repository
   it needs a token: the wizard stores it, encrypted; later sudo $TOOLS_DIR/configure.sh does.

   The scheduled power-on also depends on the BIOS: "wake on RTC" has to be enabled.
EOF
