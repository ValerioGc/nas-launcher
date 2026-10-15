#!/usr/bin/env bash
#
# Crea l'utente della console con il nome dell'account del wizard, senza password né sudo.
# La configurazione del NAS passa il nome su stdin; il socket e il relativo servizio
# restano gestiti dal launcher anche quando l'interfaccia del terminale è un'app esterna.
#
#   printf '%s' admin | sudo /usr/local/lib/nas-os/terminal-user.sh

set -euo pipefail

readonly COMMENT='nas-os terminal'
readonly DROPIN_DIR=${NAS_TERMINAL_DROPIN:-/etc/systemd/system/nas-os-terminal@.service.d}
readonly SOCKET=nas-os-terminal.socket

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

name=''
read -r name || true
[[ $name =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || fail "not a user name: ${name:-<empty>}"

if entry=$(getent passwd "$name"); then
  IFS=: read -r _ _ _ _ comment _ <<<"$entry"
  [[ $comment == "$COMMENT" ]] || fail "$name already exists and is not the user of the terminal"
else
  useradd --create-home --user-group --shell /bin/bash --comment "$COMMENT" "$name"
fi
passwd --lock "$name" >/dev/null
if id -nG "$name" | tr ' ' '\n' | grep -qx sudo; then
  gpasswd --delete "$name" sudo >/dev/null
fi

install -d -m 0755 "$DROPIN_DIR"
printf '[Service]\nUser=%s\nGroup=%s\n' "$name" "$name" >"$DROPIN_DIR/user.conf"
systemctl daemon-reload
systemctl enable --now "$SOCKET"
echo "the terminal runs as $name"
