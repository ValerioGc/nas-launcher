#!/usr/bin/env bash
#
# Checks that the installation is in order. **It reboots and powers off nothing**. The power
# permission is checked with pkcheck, which asks polkit without running the action.
#
#   scripts/check.sh
#
# It can run without root. Checking the permissions as the service user needs sudo and, without
# it, that check is skipped and says so.

set -uo pipefail

readonly SERVICE_USER=nas-os
readonly SERVICE_NAME=nas-os
readonly STATE_DIR=/var/lib/nas-os
readonly CATALOGUE=/etc/nas-os/catalogue.json
readonly APPS_DIR=/var/lib/nas-os/apps
readonly SOCKETS_DIR=/run/nas-os/apps
readonly SETTINGS=/etc/nas-os/nas-os.env
readonly LOCAL_SETTINGS=/etc/nas-os/nas-os.local.env
problems=0

setting() {
  sed -n "s/^$1=//p" "$SETTINGS" "$LOCAL_SETTINGS" 2>/dev/null | tail -n 1
}

ok() { printf '  [ok]   %s\n' "$1"; }
no() {
  printf '  [KO]   %s\n' "$1"
  problems=$((problems + 1))
}
skip() { printf '  [--]   %s\n' "$1"; }

printf '\n== Installation\n'
id -u "$SERVICE_USER" >/dev/null 2>&1 && ok "user $SERVICE_USER" || no "user $SERVICE_USER missing"
[[ -x /usr/local/bin/nas-os-axum ]] && ok "binary installed" || no "binary missing from /usr/local/bin"
[[ -f /etc/systemd/system/nas-os.service ]] && ok "unit installed" || no "unit missing"
[[ -f $SETTINGS ]] && ok "settings in $SETTINGS" || no "settings missing: $SETTINGS (run install.sh again)"
[[ -f $LOCAL_SETTINGS ]] && ok "local settings in $LOCAL_SETTINGS, which win" || skip "no local settings ($LOCAL_SETTINGS)"
[[ -f /etc/polkit-1/rules.d/10-nas-os-power.rules ]] && ok "power polkit rule installed" || no "power polkit rule missing"
[[ -f /etc/polkit-1/rules.d/20-nas-os-network.rules ]] && ok "network polkit rule installed" || no "network polkit rule missing"
[[ -f /etc/polkit-1/rules.d/30-nas-os-firewall.rules ]] && ok "access list polkit rule installed" || no "access list polkit rule missing"
[[ -f /etc/polkit-1/rules.d/40-nas-os-update.rules ]] && ok "update polkit rule installed" || no "update polkit rule missing"
[[ -f /etc/polkit-1/rules.d/50-nas-os-apps.rules ]] && ok "application polkit rule installed" || no "application polkit rule missing"
[[ -f /etc/polkit-1/rules.d/55-nas-os-app-grants.rules ]] && ok "application grants polkit rule installed" || no "application grants polkit rule missing: an application gets only what the template allows"
[[ -f /etc/polkit-1/rules.d/56-nas-os-app-packages.rules ]] && ok "application packages polkit rule installed" || no "application packages polkit rule missing: an application that needs a system package is not installed"
[[ -f /etc/polkit-1/rules.d/58-nas-os-smb-shares.rules ]] && ok "shared folders polkit rule installed" || no "shared folders polkit rule missing: no folder can be shared on the local network"
[[ -f /etc/polkit-1/rules.d/59-nas-os-cleanup.rules ]] && ok "cleanup polkit rule installed" || no "cleanup polkit rule missing: the Cleanup app cannot clean the system"
[[ -f /etc/polkit-1/rules.d/60-nas-os-apt.rules ]] && ok "system package polkit rule installed" || no "system package polkit rule missing"
[[ -f /etc/polkit-1/rules.d/65-nas-os-processes.rules ]] && ok "process polkit rule installed" || no "process polkit rule missing: the widget cannot terminate or restart"
[[ -f /etc/polkit-1/rules.d/70-nas-os-token.rules ]] && ok "release token polkit rule installed" || no "release token polkit rule missing"
[[ -f /etc/polkit-1/rules.d/75-nas-os-reset.rules ]] && ok "reset polkit rule installed" || no "reset polkit rule missing"

printf '\n== Reset\n'
[[ -f /etc/systemd/system/nas-os-reset.service ]] && ok "reset unit installed" || no "reset unit missing"
[[ -x /usr/local/lib/nas-os/factory-reset.sh ]] && ok "reset script installed" || no "reset script missing"
[[ -f /etc/systemd/system/nas-wizard.service ]] && ok "the wizard is installed, the reset can start it" || skip "the wizard is not installed: the reset is not available"

printf '\n== Service\n'
if systemctl is-active --quiet "$SERVICE_NAME"; then
  ok "service running"
else
  no "service not running (systemctl status $SERVICE_NAME)"
fi
if [[ $(setting NAS_POWER_MODE) == real ]]; then
  ok "power mode: real"
else
  skip "power mode: simulated (NAS_POWER_MODE=real missing from $SETTINGS)"
fi
[[ -f $STATE_DIR/launcher.db ]] && ok "database present in $STATE_DIR" || skip "database not created yet"

printf '\n== Reboot and shutdown permissions\n'
if ! command -v pkcheck >/dev/null; then
  skip "pkcheck not available: check skipped"
elif [[ $EUID -ne 0 ]]; then
  skip "sudo is needed to check the permissions of the service user"
else
  for action in org.freedesktop.login1.reboot org.freedesktop.login1.power-off; do
    if runuser -u "$SERVICE_USER" -- bash -c "pkcheck --action-id $action --process \$\$" >/dev/null 2>&1; then
      ok "$action allowed"
    else
      no "$action refused: the launcher would answer 503"
    fi
  done
fi

printf '\n== Network\n'
if [[ $(setting NAS_NETWORK_MODE) == real ]]; then
  ok "network mode: real"
else
  skip "network mode: read only (NAS_NETWORK_MODE=real missing from $SETTINGS)"
fi
if command -v nmcli >/dev/null; then
  ok "nmcli available: $(nmcli -t -f DEVICE,TYPE device 2>/dev/null | grep -c ':wifi$') wireless interfaces"
else
  no "nmcli missing (the wizard installs it with the system packages): listing and changing network do not work"
fi

printf '\n== Allowed addresses\n'
if systemctl is-active --quiet nas-os-base-firewall.service &&
  command -v nft >/dev/null && nft list table inet nas_base >/dev/null 2>&1; then
  ok "base firewall active"
else
  no "base firewall inactive (the wizard installs it: packages/os_configuration in its package)"
fi
if [[ $(setting NAS_FIREWALL_MODE) == real ]]; then
  ok "access list mode: real"
else
  skip "access list mode: the list holds inside the launcher only (NAS_FIREWALL_MODE=real missing from $SETTINGS)"
fi
[[ -f /etc/systemd/system/nas-os-firewall.service ]] && ok "access list unit installed" || no "access list unit missing"
[[ -x /usr/local/lib/nas-os/apply-access-rules.sh ]] && ok "access list script installed" || no "access list script missing"
if command -v nft >/dev/null; then
  if nft list table inet nas_launcher >/dev/null 2>&1; then
    ok "filter in place: $(nft list table inet nas_launcher 2>/dev/null | grep -c 'accept') accept rules"
  else
    skip "no filter in place: the list is empty, or it has never been applied"
  fi
else
  no "nft missing (the wizard installs it with the system packages): the list does not become a filter"
fi

printf '\n== Application packages\n'
if [[ $(setting NAS_PACKAGES_MODE) == real ]]; then
  ok "package mode: real"
else
  skip "package mode: the launcher reaches no repository (NAS_PACKAGES_MODE=real missing from $SETTINGS)"
fi
if [[ -f $CATALOGUE ]]; then
  entries=$(grep -c '"id"' "$CATALOGUE" 2>/dev/null || echo 0)
  ok "catalogue present: $entries applications in $CATALOGUE"
else
  no "catalogue missing from $CATALOGUE: nothing can be installed"
fi
[[ -d $APPS_DIR ]] && ok "package directory $APPS_DIR" || no "package directory $APPS_DIR missing"
command -v tar >/dev/null && ok "tar available" || no "tar missing: a package cannot be unpacked"
[[ -f /etc/systemd/system/nas-os-token.service ]] && ok "token unit installed" || no "token unit missing: the wizard and the App Manager cannot store a token"
[[ -x /usr/local/lib/nas-os/launcher-token.sh ]] && ok "token helper installed" || no "token helper missing"
launcher_repository=$(setting NAS_LAUNCHER_REPOSITORY)
if [[ -n ${launcher_repository:-} ]]; then
  ok "launcher releases: $launcher_repository"
else
  skip "no repository for the launcher itself (NAS_LAUNCHER_REPOSITORY)"
fi
[[ -f /etc/systemd/system/nas-os-update.service ]] && ok "update unit installed" || no "update unit missing"
[[ -x /usr/local/lib/nas-os/install-launcher-update.sh ]] && ok "update script installed" || no "update script missing"

printf '\n== Release token\n'
readonly CREDENTIAL=/etc/credstore.encrypted/nas-os.launcher-token
token=''
if [[ ! -f $CREDENTIAL ]]; then
  skip "no release token stored: private repositories are unavailable (wizard, App Manager or /usr/local/lib/nas-os/configure.sh)"
elif [[ $EUID -ne 0 ]]; then
  skip "a release token is stored; sudo is needed to check it"
else
  if token=$(systemd-creds decrypt --name=nas-os.launcher-token "$CREDENTIAL" - 2>/dev/null) && [[ -n $token ]]; then
    ok "the release token decrypts on this machine"
  else
    token=''
    no "the release token does not decrypt here: store it again"
  fi
  if [[ -s /run/credentials/$SERVICE_NAME.service/nas-os.launcher-token ]]; then
    ok "the launcher received the token"
  else
    no "the launcher has no token: it reads it when it starts (systemctl restart $SERVICE_NAME)"
  fi
fi

github_repositories() {
  [[ -n ${launcher_repository:-} ]] && printf '%s\n' "$launcher_repository"
  [[ -f $CATALOGUE ]] && awk '
    /"provider"/ { provider = $0 }
    /"repository"/ { repository = $0 }
    /}/ {
      if (provider ~ /github/ && repository != "") {
        gsub(/.*"repository"[ \t]*:[ \t]*"|".*/, "", repository)
        print repository
      }
      provider = ""; repository = ""
    }' "$CATALOGUE"
}

if [[ -n $token ]] && command -v curl >/dev/null; then
  while read -r repository; do
    code=$(printf 'header = "Authorization: Bearer %s"\n' "$token" |
      curl -K - -s -o /dev/null -w '%{http_code}' --max-time 10 \
        -H 'Accept: application/vnd.github+json' \
        "https://api.github.com/repos/$repository/releases/latest" || true)
    case $code in
      200) ok "$repository: the token reads its latest release" ;;
      401) no "$repository: GitHub refuses the token, expired or mistyped" ;;
      403) no "$repository: the token has no permission here (Contents: read-only)" ;;
      404) no "$repository: no release yet, or the token cannot see this repository" ;;
      *) no "$repository: GitHub answered ${code:-nothing}" ;;
    esac
  done < <(github_repositories | sort -u)
elif [[ -n $token ]]; then
  skip "curl missing: the token cannot be tried against GitHub"
fi
token=''

printf '\n== Opening the applications\n'
ok "the applications answer at /apps/<id>/ on the address of the console"
if [[ $(setting NAS_APPS_MODE) == real ]]; then
  ok "application mode: real"
else
  skip "application mode: nothing is started (NAS_APPS_MODE=real missing from $SETTINGS)"
fi
[[ -f '/etc/systemd/system/nas-app@.service' ]] && ok "application template installed" || no "template nas-app@.service missing: no application can be started"
[[ -f '/etc/systemd/system/nas-os-app-grants@.service' ]] && ok "application grants unit installed" || no "nas-os-app-grants@.service missing: no application gets the paths it asks for"
[[ -x /usr/local/lib/nas-os/apply-app-grants.sh ]] && ok "application grants helper installed" || no "apply-app-grants.sh missing"
[[ -f '/etc/systemd/system/nas-os-app-packages@.service' ]] && ok "application packages unit installed" || no "nas-os-app-packages@.service missing: no application gets the system packages it asks for"
[[ -x /usr/local/lib/nas-os/app-packages.sh ]] && ok "application packages helper installed" || no "app-packages.sh missing"
[[ -f /etc/systemd/system/nas-os-sync-ports.service && -x /usr/local/lib/nas-os/apply-sync-ports.sh ]] && ok "sync ports unit installed" || no "nas-os-sync-ports.service missing: no sync can be opened to the local network"
[[ -f /etc/polkit-1/rules.d/62-nas-os-sync-ports.rules ]] && ok "sync ports polkit rule installed" || no "sync ports polkit rule missing: no sync can be opened to the local network"
[[ -f /etc/systemd/system/nas-os-discovery.service && -x /usr/local/lib/nas-os/apply-discovery.sh ]] && ok "discovery unit installed" || no "nas-os-discovery.service missing: NAS Finder cannot be let in from the security center"
[[ -f /etc/polkit-1/rules.d/63-nas-os-discovery.rules ]] && ok "discovery polkit rule installed" || no "discovery polkit rule missing: NAS Finder cannot be let in from the security center"
[[ -f /etc/systemd/system/nas-os-cleanup.service ]] && ok "cleanup unit installed" || no "nas-os-cleanup.service missing: the Cleanup app cannot clean the system"
[[ -f /etc/systemd/system/nas-os-smb-shares.service ]] && ok "shared folders unit installed" || no "nas-os-smb-shares.service missing: no folder can be shared on the local network"
[[ -x /usr/local/lib/nas-os/apply-smb-shares.sh ]] && ok "shared folders helper installed" || no "apply-smb-shares.sh missing"
[[ -f '/etc/systemd/system/nas-os-process@.service' ]] && ok "process unit installed" || no "nas-os-process@.service missing: the widget cannot terminate or restart"
[[ -x /usr/local/lib/nas-os/act-on-process.sh ]] && ok "process helper installed" || no "act-on-process.sh missing"
[[ -d /srv/storage ]] && ok "nas-file-manager storage root exists" || no "nas-file-manager storage root /srv/storage missing"
if [[ $(stat -c '%a:%U:%G' /srv/nas-volumes 2>/dev/null) == 700:root:root ]]; then
  ok "raw volume root is private to root"
else
  no "raw volume root /srv/nas-volumes must be 0700 root:root"
fi
if [[ $(stat -c '%a:%U:%G' /srv/storage 2>/dev/null) == 710:root:nas-storage ]]; then
  ok "Explorer storage root uses the dedicated group"
else
  no "Explorer storage root /srv/storage must be 0710 root:nas-storage"
fi
getent group nas-storage >/dev/null && ok "storage group exists" || no "storage group missing"
if [[ $(setting NAS_STORAGE_MODE) == real ]]; then
  ok "storage mode: real"
else
  no "storage mode: NAS_STORAGE_MODE=real missing from $SETTINGS"
fi
[[ -f /etc/systemd/system/nas-os-storage.service ]] && ok "storage discovery unit installed" || no "storage discovery unit missing"
[[ -f /etc/systemd/system/nas-os-storage-initialize.service ]] && ok "storage initialization unit installed" || no "storage initialization unit missing"
[[ -f /etc/systemd/system/nas-os-storage-rename.service ]] && ok "storage rename unit installed" || no "storage rename unit missing"
[[ -f /etc/systemd/system/nas-os-storage-eject.service ]] && ok "storage eject unit installed" || no "storage eject unit missing: external disks cannot be ejected"
[[ -f /etc/systemd/system/nas-os-storage-smart.service ]] && ok "storage S.M.A.R.T. unit installed" || no "storage S.M.A.R.T. unit missing"
systemctl is-active --quiet nas-os-storage-crypt.socket && ok "encryption socket listening" || no "encryption socket not listening: encrypted volumes cannot be managed"
if [[ $(setting NAS_ENCRYPTION_MODE) == real ]]; then
  ok "encryption mode: real"
else
  ok "encryption mode: off (NAS_ENCRYPTION_MODE=off in nas-os.local.env)"
fi
command -v parted >/dev/null && ok "parted available for disk initialization" || no "parted missing: disks cannot be initialized"
command -v mkfs.ext4 >/dev/null && ok "mkfs.ext4 available for disk initialization" || no "e2fsprogs missing: ext4 volumes cannot be created"
[[ -f /etc/systemd/system/nas-os-storage.timer ]] && ok "storage refresh timer installed" || no "storage refresh timer missing"
[[ -f /etc/udev/rules.d/90-nas-os-storage.rules ]] && ok "storage hotplug rule installed" || no "storage hotplug rule missing"
if [[ -f /etc/nas-os/storage-volumes.tsv ]]; then
  ok "storage registry present"
  if [[ $EUID -eq 0 ]]; then
    while IFS=$'\t' read -r volume_id volume_name volume_path; do
      [[ -n $volume_id ]] || continue
      if [[ $volume_path == "/srv/storage/$volume_id" ]] && mountpoint -q "$volume_path"; then
        ok "$volume_name mounted at $volume_path"
      else
        no "$volume_id is not mounted at its Explorer path"
      fi
    done </etc/nas-os/storage-volumes.tsv
  else
    skip "run as root to verify each storage mount"
  fi
else
  skip "storage registry not generated yet"
fi
[[ -x /usr/local/lib/nas-os/run-app.sh ]] && ok "application runner installed" || no "run-app.sh missing: the template would have nothing to execute"
closed=$(find "$APPS_DIR" -mindepth 1 \( -type d ! -perm -g=rx -o ! -perm -g=r \) 2>/dev/null | wc -l)
[[ $closed -eq 0 ]] && ok "the packages are readable by their processes" || no "$closed package files the application processes cannot read: run install.sh again"
active=$(systemctl list-units --plain --no-legend 'nas-app@*.service' 2>/dev/null | grep -c ' active ' || true)
skip "$active application units active (systemctl status 'nas-app@*')"
if [[ -d $SOCKETS_DIR ]]; then
  listening=$(find "$SOCKETS_DIR" -maxdepth 1 -name '*.sock' 2>/dev/null | wc -l)
  ok "socket directory $SOCKETS_DIR: $listening applications listening"
  installed=$(find "$APPS_DIR" -maxdepth 2 -name nas-app.json 2>/dev/null | wc -l)
  [[ $installed -gt $listening ]] && skip "$((installed - listening)) installed applications are not running: they would answer 503"
else
  no "socket directory $SOCKETS_DIR missing: an installed application cannot be opened"
fi

printf '\n== System packages\n'
if [[ $(setting NAS_APT_MODE) == real ]]; then
  ok "system package mode: real"
else
  skip "system package mode: read only (NAS_APT_MODE=real missing from $SETTINGS)"
fi
[[ -f '/etc/systemd/system/nas-os-apt@.service' ]] && ok "package template installed" || no "template nas-os-apt@.service missing: no upgrade can be installed"
[[ -x /usr/local/lib/nas-os/apt-updates.sh ]] && ok "package helper installed" || no "apt-updates.sh missing: the template would have nothing to execute"

printf '\n== Terminal\n'
if [[ $(setting NAS_TERMINAL_MODE) == real ]]; then
  ok "terminal mode: real"
else
  skip "terminal mode: no shell is opened (NAS_TERMINAL_MODE=real missing from $SETTINGS)"
fi
command -v bash >/dev/null && ok "bash available for the terminal" || no "bash missing: the terminal cannot open a shell"
if systemctl is-active --quiet nas-os-terminal.socket; then
  ok "terminal socket listening as $(systemctl show 'nas-os-terminal@.service' -p User --value 2>/dev/null)"
else
  no "terminal socket off: the user of the terminal was not created (the wizard creates it: /usr/local/lib/nas-os/configure.sh --user <name>)"
fi
if id -nG "$SERVICE_USER" 2>/dev/null | grep -qw systemd-journal || systemctl show "$SERVICE_NAME" -p SupplementaryGroups --value 2>/dev/null | grep -qw systemd-journal; then
  ok "the service can read the system journal for the log"
else
  no "the service cannot read the system journal: the log of the terminal would stay empty"
fi

printf '\n== Scheduled power-on\n'
[[ -f /etc/systemd/system/nas-os-wake.service ]] && ok "wake alarm unit installed" || no "wake alarm unit missing: the power-on rules would do nothing"
[[ -x /usr/local/lib/nas-os/wake-alarm.sh ]] && ok "wake alarm script installed" || no "wake alarm script missing"
if [[ -e /sys/class/rtc/rtc0/wakealarm ]]; then
  ok "the clock exposes its alarm"
  [[ -s /sys/class/rtc/rtc0/wakealarm ]] && skip "alarm currently set: $(date -d "@$(cat /sys/class/rtc/rtc0/wakealarm)" '+%F %H:%M:%S' 2>/dev/null)" || skip "no alarm set at the moment"
else
  no "this machine does not expose /sys/class/rtc/rtc0/wakealarm: a scheduled power-on is not possible"
fi
[[ -f $STATE_DIR/wakealarm ]] && skip "last alarm asked for by the launcher: $(date -d "@$(cat $STATE_DIR/wakealarm 2>/dev/null)" '+%F %H:%M:%S' 2>/dev/null)" || skip "the launcher has not asked for an alarm yet"

answers() {
  local name=$1 url=$2 code=''
  if command -v curl >/dev/null; then
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "$url" || true)
  elif command -v wget >/dev/null; then
    code=$(wget -S -O /dev/null -T 3 -t 1 "$url" 2>&1 | awk '/HTTP\//{code=$2} END{print code}')
  else
    skip "neither curl nor wget available"
    return
  fi
  [[ $code == 200 ]] && ok "$name answers 200" || no "$name does not answer (code: ${code:-none})"
}

printf '\n== Answer from the launcher\n'
answers "/health" http://127.0.0.1:3000/health

printf '\n== Interface\n'
[[ -f /usr/local/share/nas-os/ui/index.html ]] && ok "interface installed" || no "interface missing from /usr/local/share/nas-os/ui"
if systemctl is-active --quiet nas-os-gateway; then
  answers "the interface on port 80" http://127.0.0.1/
else
  skip "the gateway is not running: /usr/local/lib/nas-os-gateway/check.sh says why"
fi

printf '\n'
if [[ $problems -eq 0 ]]; then
  printf 'All in order.\n'
else
  printf '%d things to fix. Run again: sudo ./install.sh\n' "$problems"
  exit 1
fi
