#!/usr/bin/env bash
# Provisions the terminal account using the wizard account name from standard input.
# The account has no password or sudo privileges.
# The launcher owns the terminal socket and service.
# The terminal interface is provided by an external application.

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
