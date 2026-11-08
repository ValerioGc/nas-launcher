#!/usr/bin/env bash
# Resets launcher, gateway, and application state before starting the setup wizard.
# Removes the terminal account and home directory and clears private release tokens.
# Preserves data disks, user storage under /srv, and encrypted data-disk keys.
# The machine ends as the entry script of the wizard left it before the first configuration. That
# is the original configuration of Debian, then the configuration of the system the wizard applies
# again (its packages, the wireless check, the screen turned off and the base firewall), and the
# wizard running. The packages the applications added go, upgrades of the system stay.
# The service user, its groups and the console tools stay, the files on the data disks keep their
# owners.
# Runs as root through nas-os-reset.service after gateway password verification.
# An available verified wizard update is installed first, with fallback to the
# installed wizard. The launcher stops during reset and diagnostics remain in the journal.

set -euo pipefail

# Overridable so the script can be tested without root and without touching the machine.
readonly STATE=${NAS_STATE_DIR:-/var/lib/nas-os}
readonly GATEWAY_STATES=(${NAS_GATEWAY_STATES:-/var/lib/nas-os-gateway /var/lib/private/nas-os-gateway})
readonly APP_STATES=(${NAS_APP_STATES:-/var/lib/nas-app /var/lib/private/nas-app})
readonly CREDENTIAL=${NAS_CREDENTIAL:-/etc/credstore.encrypted/nas-os.launcher-token}
readonly TERMINAL_DROPIN=${NAS_TERMINAL_DROPIN:-/etc/systemd/system/nas-os-terminal@.service.d}
readonly WIZARD_UNIT=${NAS_WIZARD_UNIT:-/etc/systemd/system/nas-wizard.service}
readonly WIZARD_STATE=${NAS_WIZARD_STATE:-/var/lib/nas-wizard}
readonly WIZARD_UPDATES=${NAS_WIZARD_UPDATES:-/var/lib/nas-wizard-updates}
readonly RUNTIME=${NAS_RUNTIME_DIR:-/run/nas-os}
readonly APP_DROPINS=${NAS_APP_DROPINS:-/etc/systemd/system}
readonly STORAGE_RULE=${NAS_STORAGE_RULE:-/etc/udev/rules.d/90-nas-os-storage.rules}
readonly ANTIVIRUS_STATE=${NAS_ANTIVIRUS_STATE_DIR:-${STATE%/nas-os}/nas-os-antivirus}
readonly WIZARD_UPDATE=$STATE/wizard-update.tar
readonly TERMINAL_COMMENT='nas-os terminal'
readonly WIZARD_HOME=${NAS_WIZARD_HOME:-/usr/local/lib/nas-wizard}
readonly RESTORE=${NAS_CONFIGURATION_HELPER:-$WIZARD_HOME/packages/os_configuration/scripts/uninstall.sh}
readonly LAUNCHER_TOOLS=${NAS_LAUNCHER_TOOLS:-/usr/local/lib/nas-os}
readonly GATEWAY_TOOLS=${NAS_GATEWAY_TOOLS:-/usr/local/lib/nas-os-gateway}
readonly CONFIGURE=$WIZARD_HOME/packages/os_configuration/install.sh
readonly PENDING_UPDATE=$STATE/update-verify/pending
# The sets of the base firewall the launcher fills, emptied so nothing it opened stays open.
readonly NAS_SETS=(nas_shares nas_sync nas_discovery nas_smb_servers nas_smb_servers6 nas_apps_out
  nas_apps_smtp nas_apps_sync)

say() { printf '\n== %s\n' "$1"; }
fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

[[ $EUID -eq 0 || -n ${NAS_STATE_DIR:-} ]] || fail "it has to run as root: nas-os-reset.service runs it"
# Requires an installed wizard before any reset data is removed.
[[ -f $WIZARD_UNIT ]] || fail "the wizard is not installed ($WIZARD_UNIT): nothing was removed"
[[ -x $WIZARD_HOME/wizard/bin/nas-wizard && -x $RESTORE ]] || fail 'the wizard or system restoration helper is missing: nothing was removed'
[[ -f $CONFIGURE ]] || fail 'the configuration of the system is missing, nothing was removed'
if command -v nft >/dev/null; then
  nft list tables >/dev/null || fail 'cannot inspect nftables rules, nothing was removed'
fi
[[ -x $LAUNCHER_TOOLS/uninstall.sh && -x $GATEWAY_TOOLS/uninstall.sh ]] || fail 'the launcher or gateway removal helper is missing: nothing was removed'
"$RESTORE" --check
busy=$(systemctl list-units --plain --no-legend --state=active,activating,deactivating,reloading \
  'nas-os-update.service' 'nas-os-update-verify.service' 'nas-os-apt@*.service' 'nas-os-app-packages@*.service' \
  'nas-os-storage-initialize.service' 'nas-os-storage-rename.service' 'nas-os-storage-eject.service' \
  'nas-os-storage-crypt@*.service')
[[ -z $busy ]] || fail 'wait for NAS updates, package installation or disk operations before resetting'
# A check after an update would put the previous version back in the middle of the reset.
[[ ! -e $PENDING_UPDATE ]] || fail 'an update waits for its check after the restart, restart the NAS first, nothing was removed'

stop_unit() {
  local unit=$1 load
  shift
  load=$(systemctl show "$unit" --property=LoadState --value)
  if [[ $load != not-found ]]; then systemctl "$@" "$unit"; fi
}

say "Update of the wizard"
if [[ -r $WIZARD_UPDATE ]]; then
  staging=$(mktemp -d)
  if tar --extract --auto-compress --no-same-owner --file "$WIZARD_UPDATE" --directory "$staging" &&
    [[ -f $staging/install.sh ]] && bash "$staging/install.sh" --update; then
    echo "   the wizard was updated"
  else
    echo "   warning: the update of the wizard failed, the one installed starts" >&2
  fi
  rm -rf -- "$staging" "$WIZARD_UPDATE"
else
  echo "   no newer wizard"
fi
[[ -x $WIZARD_HOME/wizard/bin/nas-wizard ]] || fail 'the wizard update left no executable wizard: reset cancelled'
"$RESTORE" --check

say "Gateway"
# Disables the gateway until wizard completion to preserve setup access on port 80 after reboot.
stop_unit nas-os-gateway.service disable --now
stop_unit nas-os-share-password.socket disable --now
rm -f -- "${NAS_SHARE_PASSWORD_SOCKET:-${RUNTIME%/*}/nas-os-share-password.sock}"

say "Applications"
listing=$(systemctl list-units --all --plain --no-legend 'nas-app@*.service' \
  'nas-os-terminal@*.service' 'nas-os-share-password@*.service' 'nas-os-app-grants@*.service')
mapfile -t instances < <(printf '%s\n' "$listing" | awk 'NF { print $1 }')
if ((${#instances[@]} > 0)); then
  systemctl stop "${instances[@]}"
fi
echo "   ${#instances[@]} NAS process units stopped"
# The paths the launcher granted them go with them.
rm -rf -- "$APP_DROPINS"/nas-app@?*.service.d

say "Launcher"
stop_unit nas-os.service disable --now
rm -f -- "$STORAGE_RULE"
udevadm control --reload-rules
stop_unit nas-os-storage.timer disable --now
stop_unit nas-os-storage.service stop
stop_unit nas-os-hostname.service stop
# Optional antivirus work outlives a launcher restart, but must stop before its state is reset.
stop_unit nas-os-antivirus-lifecycle.service disable --now
stop_unit nas-os-antivirus.service stop
stop_unit nas-os-cleanup.service stop
if [[ -f $STATE/app-packages/nas-security-center ]]; then
  stop_unit clamav-freshclam.service disable --now
fi

say "Network access and shared folders"
stop_unit nas-os-firewall.service stop
stop_unit nas-os-smb-shares.service stop
stop_unit nas-os-sync-ports.service stop
stop_unit nas-os-discovery.service stop
stop_unit smbd.service stop
stop_unit nmbd.service stop
# The base firewall stays loaded, as before the first configuration, so the wizard is never open
# to more than its own port and SSH. Only what the launcher opened in it goes.
if command -v nft >/dev/null; then
  tables=$(nft list tables) || fail 'cannot inspect nftables rules'
  if grep -qxF 'table inet nas_launcher' <<<"$tables"; then nft delete table inet nas_launcher; fi
  if grep -qxF 'table inet nas_base' <<<"$tables"; then
    for set in "${NAS_SETS[@]}"; do
      nft flush set inet nas_base "$set" 2>/dev/null || true
    done
  fi
fi

say "Screen"
# Off, as the configuration of the system installs it. tty1 goes back to its login.
stop_unit nas-os-screen.service disable --now
stop_unit nas-os-screen-browser.service stop

say "User of the terminal"
stop_unit nas-os-terminal.socket disable --now
rm -f -- "${NAS_TERMINAL_SOCKET:-${RUNTIME%/*}/nas-os-terminal.sock}"
user=''
if [[ -f $TERMINAL_DROPIN/user.conf ]]; then
  user=$(sed -n 's/^User=//p' "$TERMINAL_DROPIN/user.conf" | tail -n 1)
fi
if [[ $user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] &&
  [[ $(getent passwd "$user" | cut -d: -f5) == "$TERMINAL_COMMENT" ]]; then
  # Stops account processes before removing the terminal user.
  "$RESTORE" --remove-terminal-user "$user"
else
  echo "   no user of the terminal to remove"
fi
rm -rf -- "$TERMINAL_DROPIN"

say "Installed NAS components"
# Leaves data and account removal to the explicit reset steps rather than uninstall --purge.
"$LAUNCHER_TOOLS/uninstall.sh"
"$GATEWAY_TOOLS/uninstall.sh"

say "Original system configuration"
# Also restores the Samba configuration and credentials from before the NAS.
"$RESTORE" --remove-app-packages
"$RESTORE" --restore

say "Data"
rm -f -- "$CREDENTIAL"
# Removes network-share credentials while retaining encrypted data-disk keys.
rm -f -- "${CREDENTIAL%/*}"/nas-os.smb-*
rm -rf -- "$STATE" "${GATEWAY_STATES[@]}" "${APP_STATES[@]}" "$WIZARD_STATE" "$WIZARD_UPDATES"
rm -rf -- "$ANTIVIRUS_STATE" "${NAS_LOG_DIR:-/var/log/nas-os}" "$RUNTIME"
rm -f -- "${RUNTIME%/*}/nas-os-antivirus.lock" "${RUNTIME%/*}/nas-os-cleanup.lock"
echo "   launcher, gateway, applications, token and network disk passwords removed; the data disks are untouched"

say "Configuration of the system"
# Applied again over the original configuration, as the entry script does before the first
# configuration. The wizard applies it once more before it installs the NAS, so a failure here
# does not keep it from starting.
bash "$CONFIGURE" || echo '   warning: the configuration of the system could not be applied again, the wizard applies it' >&2

say "Wizard"
systemctl daemon-reload
systemctl enable --now nas-wizard.service || fail "the wizard did not start (journalctl -u nas-wizard)"
systemctl is-active --quiet nas-wizard.service || fail 'the wizard did not remain active'
echo "   the original system configuration is restored and the wizard is running"
