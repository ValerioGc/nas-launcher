#!/usr/bin/env bash
#
# Takes the NAS back to where the wizard found it, and starts the wizard again: the launcher, the
# gateway and the applications lose every piece of data they kept, the user of the terminal goes
# with its home, the token of the private releases is forgotten. The data disks are not touched:
# nothing under /srv is removed, and the areas of the users on them stay as they are.
#
# It runs as root from `nas-os-reset.service`, which the launcher starts once the gateway has
# checked the password of the account. The launcher stops along the way, so nothing reports the
# outcome but the journal: journalctl -u nas-os-reset.
#
# When the launcher found a newer wizard, it left its release, checked, in the state directory: it
# is installed first (`install.sh --update`), so the NAS is configured again with the latest. If
# that fails, the reset goes on with the wizard already installed.
#
#   systemctl start nas-os-reset.service

set -uo pipefail

# Overridable so the script can be tested without root and without touching the machine.
readonly STATE=${NAS_STATE_DIR:-/var/lib/nas-os}
readonly GATEWAY_STATES=(${NAS_GATEWAY_STATES:-/var/lib/nas-os-gateway /var/lib/private/nas-os-gateway})
readonly APP_STATES=(${NAS_APP_STATES:-/var/lib/nas-app /var/lib/private/nas-app})
readonly CREDENTIAL=${NAS_CREDENTIAL:-/etc/credstore.encrypted/nas-os.launcher-token}
readonly TERMINAL_DROPIN=${NAS_TERMINAL_DROPIN:-/etc/systemd/system/nas-os-terminal@.service.d}
readonly WIZARD_UNIT=${NAS_WIZARD_UNIT:-/etc/systemd/system/nas-wizard.service}
readonly WIZARD_STATE=${NAS_WIZARD_STATE:-/var/lib/nas-wizard}
readonly APP_DROPINS=${NAS_APP_DROPINS:-/etc/systemd/system}
readonly ANTIVIRUS_STATE=${NAS_ANTIVIRUS_STATE_DIR:-${STATE%/nas-os}/nas-os-antivirus}
readonly WIZARD_UPDATE=$STATE/wizard-update.tar
readonly TERMINAL_COMMENT='nas-os terminal'

say() { printf '\n== %s\n' "$1"; }
fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

[[ $EUID -eq 0 || -n ${NAS_STATE_DIR:-} ]] || fail "it has to run as root: nas-os-reset.service runs it"
# Without the wizard nothing would configure the NAS again: nothing is removed.
[[ -f $WIZARD_UNIT ]] || fail "the wizard is not installed ($WIZARD_UNIT): nothing was removed"

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

say "Gateway"
# Disabled as well: a reboot before the wizard is done must not hand port 80 back to it.
systemctl disable --now nas-os-gateway.service || true

say "Applications"
mapfile -t instances < <(systemctl list-units --plain --no-legend 'nas-app@*.service' 2>/dev/null |
  awk '{ print $1 }')
if ((${#instances[@]} > 0)); then
  systemctl stop "${instances[@]}" || true
fi
echo "   ${#instances[@]} applications stopped"
# The paths the launcher granted them go with them.
rm -rf -- "$APP_DROPINS"/nas-app@?*.service.d

say "Launcher"
systemctl stop nas-os.service || true
# Optional antivirus work outlives a launcher restart, but must stop before its state is reset.
systemctl disable --now nas-os-antivirus-lifecycle.service >/dev/null 2>&1 || true
rm -f "$ANTIVIRUS_STATE/settings.json" "$ANTIVIRUS_STATE/boot-id" "$ANTIVIRUS_STATE/notices.json"
systemctl stop nas-os-antivirus.service || true
systemctl stop nas-os-cleanup.service || true
if [[ -f $STATE/app-packages/nas-security-center ]]; then
  systemctl disable --now clamav-freshclam.service || true
fi

say "User of the terminal"
systemctl disable --now nas-os-terminal.socket || true
user=$(sed -n 's/^User=//p' "$TERMINAL_DROPIN/user.conf" 2>/dev/null | tail -n 1)
if [[ $user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] &&
  [[ $(getent passwd "$user" | cut -d: -f5) == "$TERMINAL_COMMENT" ]]; then
  # Its processes first: a user with a running shell cannot be removed.
  pkill -KILL -u "$user" || true
  userdel --remove "$user" && echo "   $user removed, with its home"
else
  echo "   no user of the terminal to remove"
fi
rm -rf -- "$TERMINAL_DROPIN"

say "Data"
rm -f -- "$CREDENTIAL"
rm -rf -- "$STATE" "${GATEWAY_STATES[@]}" "${APP_STATES[@]}" "$WIZARD_STATE"
echo "   launcher, gateway, applications and token removed; the data disks are untouched"

say "Wizard"
systemctl daemon-reload
systemctl start nas-wizard.service || fail "the wizard did not start (journalctl -u nas-wizard)"
echo "   the wizard answers on port 80"
