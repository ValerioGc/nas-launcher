#!/usr/bin/env bash
#
# Configures the launcher once it is installed (see ../ui_installation). It runs the helpers the
# installation put down, the same ones the launcher starts, so each thing is done one way only.
#
# By hand, for the shared token of the private launcher and application releases, which the App
# Manager can also set:
#
#   sudo ./install.sh              # asks for the token, without showing it
#   sudo ./install.sh --clear      # removes it
#
# For the wizard, which runs it once, as root, after the installation: the token on the input (an
# empty input stores none), then the user of the terminal named after the account, then the data
# volumes of that user.
#
#   printf '%s' "$TOKEN" | sudo ./install.sh --user <name>
#
# The installation also copies it to /usr/local/lib/nas-os/configure.sh, where it stays once the
# package is gone.

set -euo pipefail

# Overridable so the script can be tested against the helpers in the repository.
readonly HELPER=${NAS_TOKEN_HELPER:-/usr/local/lib/nas-os/launcher-token.sh}
readonly TERMINAL_HELPER=${NAS_TERMINAL_HELPER:-/usr/local/lib/nas-os/terminal-user.sh}
readonly STORAGE_UNIT=nas-os-storage.service

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

if [[ ! -x $HELPER ]]; then
  fail "$HELPER is missing: install the launcher first (ui_installation)"
fi

[[ ${1:-} == --user ]] || exec "$HELPER" "$@"

name=${2:-}
(($# == 2)) || fail 'usage: install.sh --user <name>, with the token on the input'
[[ -x $TERMINAL_HELPER ]] || fail "$TERMINAL_HELPER is missing: install the launcher first"

# The whole input, read before anything else: the helpers read their own input too.
token=$(cat)
if [[ -n $token ]]; then
  printf '%s' "$token" | "$HELPER"
else
  echo 'no token: the private repositories stay unreachable until one is given'
fi
token=''

printf '%s' "$name" | "$TERMINAL_HELPER"

# The volumes are published in the area of the user just created: a reconciliation done before
# it existed had nobody to publish them for.
systemctl start "$STORAGE_UNIT" || fail "$STORAGE_UNIT did not complete (journalctl -u $STORAGE_UNIT)"
echo 'data volumes reconciled'
