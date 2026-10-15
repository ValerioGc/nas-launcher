#!/usr/bin/env bash
#
# Removes the launcher from the machine. **The data stays**: the database in /var/lib/nas-os is not
# deleted unless --purge asks for it, and even then a confirmation is required.
#
#   sudo scripts/uninstall.sh [--purge]

set -euo pipefail

readonly SERVICE_USER=nas-os
readonly SERVICE_NAME=nas-os
readonly STATE_DIR=/var/lib/nas-os
readonly RESTORE=${NAS_CONFIGURATION_HELPER:-/usr/local/lib/nas-wizard/packages/os_configuration/scripts/uninstall.sh}
terminal_user=$(sed -n 's/^User=//p' /etc/systemd/system/nas-os-terminal@.service.d/user.conf 2>/dev/null | tail -n 1) || terminal_user=''
purge=false
[[ ${1:-} == --purge ]] && purge=true

say() { printf '\n== %s\n' "$1"; }
fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

[[ $EUID -eq 0 ]] || fail "it has to run as root: sudo $0 ${1:-}"
[[ -x $RESTORE ]] || fail 'the system restoration helper is missing'
"$RESTORE" --check

stop_unit() {
  local unit=$1 load
  shift
  load=$(systemctl show "$unit" --property=LoadState --value)
  if [[ $load != not-found ]]; then systemctl "$@" "$unit"; fi
}

say "Service"
# Read the entire listing: grep -q can close the pipe early and make pipefail skip the stop.
if systemctl list-unit-files --no-pager --no-legend | grep "^$SERVICE_NAME\.service[[:space:]]" >/dev/null; then
  systemctl disable --now "$SERVICE_NAME"
  rm -f /etc/systemd/system/nas-os.service
  systemctl daemon-reload
  echo "   stopped, disabled and unit removed"
else
  echo "   it was not installed"
fi

stop_unit nas-os-hostname.service stop
"$RESTORE" --restore-hostname
rm -f /etc/systemd/system/nas-os-hostname.service /usr/local/lib/nas-os/apply-hostname.sh
rm -f /etc/systemd/system/nas-os-reset.service /usr/local/lib/nas-os/factory-reset.sh

say "Applications"
stop_unit nas-os-storage.timer disable --now
stop_unit nas-os-storage.service stop
rm -f /etc/systemd/system/nas-os-storage.service /etc/systemd/system/nas-os-storage-initialize.service /etc/systemd/system/nas-os-storage-rename.service /etc/systemd/system/nas-os-storage-eject.service /etc/systemd/system/nas-os-storage-smart.service \
  /etc/systemd/system/nas-os-storage.timer \
  /etc/udev/rules.d/90-nas-os-storage.rules
udevadm control --reload >/dev/null 2>&1 || true
# Mounted data and the registry stay in place: uninstalling the UI must not remove user files.
# Stopped first: an application is a process that outlives the launcher, and removing the
# template under a running instance would leave it nothing to be restarted from.
listing=$(systemctl list-units --all --plain --no-legend 'nas-app@*.service' \
  'nas-os-terminal@*.service' 'nas-os-app-grants@*.service')
mapfile -t instances < <(printf '%s\n' "$listing" | awk 'NF { print $1 }')
if [[ ${#instances[@]} -gt 0 ]]; then
  systemctl stop "${instances[@]}"
  echo "   ${#instances[@]} applications stopped"
else
  echo "   no application was running"
fi
rm -f '/etc/systemd/system/nas-app@.service' /usr/local/lib/nas-os/run-app.sh
# The grants the launcher wrote, one drop-in per application, with the unit that wrote them.
rm -rf /etc/systemd/system/nas-app@?*.service.d
rm -f '/etc/systemd/system/nas-os-app-grants@.service' /usr/local/lib/nas-os/apply-app-grants.sh
rm -f '/etc/systemd/system/nas-os-app-packages@.service' /usr/local/lib/nas-os/app-packages.sh
echo "   template and runner removed (the packages stay in /var/lib/nas-os/apps)"
rm -f '/etc/systemd/system/nas-os-process@.service' /usr/local/lib/nas-os/act-on-process.sh

stop_unit nas-os-antivirus-lifecycle.service disable --now
stop_unit nas-os-antivirus.service stop
if [[ -f /var/lib/nas-os/app-packages/nas-security-center ]]; then
  stop_unit clamav-freshclam.service disable --now
fi
rm -f /etc/systemd/system/nas-os-antivirus-lifecycle.service /etc/systemd/system/nas-os-antivirus.service /etc/polkit-1/rules.d/57-nas-os-antivirus.rules
rm -f /etc/systemd/system/clamav-freshclam.service.d/nas-os-firewall.conf
rmdir /etc/systemd/system/clamav-freshclam.service.d 2>/dev/null || true

say "Shared folders"
stop_unit nas-os-smb-shares.service stop
# Nothing is shared any more: port 445 closes, Samba stops, and what Debian had written in its
# configuration goes back.
nft flush set inet nas_base nas_shares >/dev/null 2>&1 || true
if head -n 1 /etc/samba/smb.conf 2>/dev/null | grep -qF '# Written by apply-smb-shares.sh'; then
  stop_unit smbd.service disable --now
  stop_unit nmbd.service disable --now
  if [[ -f /etc/samba/smb.conf.nas-os-original ]]; then
    mv -f /etc/samba/smb.conf.nas-os-original /etc/samba/smb.conf
  fi
fi
rm -f /etc/systemd/system/nas-os-smb-shares.service /usr/local/lib/nas-os/apply-smb-shares.sh
echo "   no folder shared any more"

say "Sync ports"
stop_unit nas-os-sync-ports.service stop
stop_unit nas-os-discovery.service stop
nft flush set inet nas_base nas_sync >/dev/null 2>&1 || true
rm -f /etc/systemd/system/nas-os-sync-ports.service /usr/local/lib/nas-os/apply-sync-ports.sh
rm -f /etc/polkit-1/rules.d/62-nas-os-sync-ports.rules
rm -f /etc/systemd/system/nas-os-discovery.service /usr/local/lib/nas-os/apply-discovery.sh
rm -f /etc/polkit-1/rules.d/63-nas-os-discovery.rules
echo "   closed and removed"

say "Cleanup"
stop_unit nas-os-cleanup.service stop
rm -f /etc/systemd/system/nas-os-cleanup.service /etc/polkit-1/rules.d/59-nas-os-cleanup.rules
echo "   removed"

say "System packages"
rm -f '/etc/systemd/system/nas-os-apt@.service' /usr/local/lib/nas-os/apt-updates.sh
echo "   removed"

say "Terminal"
# The user and its home stay. They hold what was done from the terminal.
stop_unit nas-os-terminal.socket disable --now
rm -f /run/nas-os-terminal.sock
rm -rf '/etc/systemd/system/nas-os-terminal@.service.d'
rm -f /etc/systemd/system/nas-os-terminal.socket '/etc/systemd/system/nas-os-terminal@.service' \
  /etc/systemd/system/nas-os-terminal-user.service /usr/local/lib/nas-os/terminal-user.sh
echo "   units removed (the user of the terminal and its home stay)"

say "Allowed addresses"
# The filter goes before the unit that knows how to remove it, otherwise it would stay active with
# nobody able to remove it: a forgotten table that keeps dropping packets.
if command -v nft >/dev/null; then
  tables=$(nft list tables) || fail 'cannot inspect nftables rules'
  if grep -qxF 'table inet nas_launcher' <<<"$tables"; then
    nft delete table inet nas_launcher
    echo "   nftables filter removed"
  fi
elif [[ -f /etc/systemd/system/nas-os-firewall.service ]]; then
  fail 'nft is missing: cannot verify removal of the access filter'
fi
stop_unit nas-os-firewall.service stop
rm -f /etc/systemd/system/nas-os-firewall.service \
  /usr/local/lib/nas-os/apply-access-rules.sh

say "Update helper"
rm -f /etc/systemd/system/nas-os-update.service \
  /usr/local/lib/nas-os/install-launcher-update.sh
echo "   removed"

say "Token helper"
# The token itself stays: ../../uninstall.sh clears it, and a reinstallation finds it again.
rm -f /etc/systemd/system/nas-os-token.service /usr/local/lib/nas-os/launcher-token.sh
echo "   removed"


say "Wake alarm"
rm -f /etc/systemd/system/nas-os-wake.service /usr/local/lib/nas-os/wake-alarm.sh
rmdir /usr/local/lib/nas-os 2>/dev/null || true
systemctl daemon-reload
# An alarm already set stays in the clock: the machine may power on once more. Clearing it here
# would avoid the surprise, but it would also clear an alarm somebody else set.
if [[ -r /sys/class/rtc/rtc0/wakealarm && -s /sys/class/rtc/rtc0/wakealarm ]]; then
  echo "   unit removed. Careful: there is still an alarm set in the clock"
  echo "   to clear it:  echo 0 | sudo tee /sys/class/rtc/rtc0/wakealarm"
else
  echo "   unit removed"
fi

say "Permissions"
rm -f /etc/polkit-1/rules.d/10-nas-os-power.rules
rm -f /etc/polkit-1/rules.d/20-nas-os-network.rules
rm -f /etc/polkit-1/rules.d/30-nas-os-firewall.rules
rm -f /etc/polkit-1/rules.d/40-nas-os-update.rules
rm -f /etc/polkit-1/rules.d/50-nas-os-apps.rules /etc/polkit-1/rules.d/55-nas-os-app-grants.rules
rm -f /etc/polkit-1/rules.d/56-nas-os-app-packages.rules
rm -f /etc/polkit-1/rules.d/58-nas-os-smb-shares.rules
rm -f /etc/polkit-1/rules.d/60-nas-os-apt.rules
rm -f /etc/polkit-1/rules.d/65-nas-os-processes.rules
rm -f /etc/polkit-1/rules.d/70-nas-os-token.rules
rm -f /etc/polkit-1/rules.d/80-nas-os-terminal.rules
rm -f /etc/polkit-1/rules.d/90-nas-os-storage.rules
rm -f /etc/polkit-1/rules.d/95-nas-os-hostname.rules
rm -f /etc/polkit-1/rules.d/75-nas-os-reset.rules
load=$(systemctl show polkit.service --property=LoadState --value)
if [[ $load != not-found ]]; then systemctl try-restart polkit.service; fi
echo "   polkit rules removed"

say "Binary"
rm -f /usr/local/bin/nas-os-axum /usr/local/bin/nas-os-axum.previous
rm -rf /usr/local/share/nas-os
# This very script among them: bash has it open already, and goes on reading it.
rm -f /usr/local/lib/nas-os/check.sh /usr/local/lib/nas-os/uninstall.sh \
  /usr/local/lib/nas-os/configure.sh
rmdir /usr/local/lib/nas-os 2>/dev/null || true
echo "   binary, interface and tools removed"

if $purge; then
  say "Data and user"
  printf '   About to delete %s, %s, the data of the applications and the user %s.\n' \
    "$STATE_DIR" /etc/nas-os "$SERVICE_USER"
  read -r -p "   Type 'delete' to confirm: " answer
  if [[ $answer == delete ]]; then
    # The applications run as dynamic users, whose state systemd keeps under /var/lib/private. The
    # account and the sessions are the gateway's, removed with it.
    rm -rf "$STATE_DIR" /etc/nas-os /var/lib/nas-app /var/lib/private/nas-app
    rm -rf /var/lib/nas-os-antivirus /var/log/nas-os /run/nas-os
    rm -f /run/nas-os-antivirus.lock /run/nas-os-cleanup.lock
    # The Debian packages installed for the applications go with them; the rest of the machine's
    # packages are never in this record.
    "$RESTORE" --remove-app-packages
    if [[ $terminal_user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]]; then
      "$RESTORE" --remove-terminal-user "$terminal_user"
    fi
    existed=$("$RESTORE" --user-existed "$SERVICE_USER")
    if [[ $existed == no ]] && id -u "$SERVICE_USER" >/dev/null 2>&1; then userdel "$SERVICE_USER"; fi
    echo "   data and user removed"
  else
    echo "   cancelled: data and user stay"
  fi
else
  say "Data"
  echo "   $STATE_DIR stays where it is. To remove it: sudo $0 --purge"
fi

printf '\nDone.\n'
