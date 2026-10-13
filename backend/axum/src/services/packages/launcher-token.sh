#!/usr/bin/env bash
#
# Stores the shared token for private launcher and app releases, encrypted
# with the key of this machine (and its TPM, where there is one) in the systemd credential store.
# systemd decrypts it for the launcher only, at every start. 
# The token is not in the database, not in a file anybody can read, and not in the interface.
#
#   sudo /usr/local/lib/nas-os/launcher-token.sh       # asks for the token, without showing it
#   printf '%s' "$TOKEN" | sudo /usr/local/lib/nas-os/launcher-token.sh
#   sudo /usr/local/lib/nas-os/launcher-token.sh --clear   # removes it
#
# ../ui_configuration/install.sh runs it by hand. nas-os-token.service runs it for the setup wizard
# and the App Manager, with the file the launcher wrote as its input.
# A GitHub token needs read access to the listed private repositories

set -euo pipefail
# Whatever this writes, the credential included, is for root alone from the start.
umask 077

readonly NAME=nas-os.launcher-token

readonly CREDSTORE=${NAS_CREDSTORE:-/etc/credstore.encrypted}
readonly SERVICE=nas-os

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

[[ $EUID -eq 0 || -n ${NAS_CREDSTORE:-} ]] || fail "it has to run as root: sudo $0"
command -v systemd-creds >/dev/null || fail 'systemd-creds is not available on this machine'

case ${1:-} in
  --clear)
    rm -f "$CREDSTORE/$NAME"
    echo "token removed"
    ;;
  '')
    token=''
    if [[ -t 0 ]]; then
      read -r -s -p 'Token for private releases: ' token
      echo
    else
      read -r token || true
    fi
    [[ -n $token && $token != *[[:space:]]* && ${#token} -le 512 ]] ||
      fail 'that does not look like a token'
    install -d -m 0700 "$CREDSTORE"
    printf '%s' "$token" | systemd-creds encrypt --name="$NAME" - "$CREDSTORE/$NAME"
    chmod 0600 "$CREDSTORE/$NAME"
    echo "token stored, encrypted, in $CREDSTORE/$NAME"
    ;;
  *)
    fail "unknown option: $1"
    ;;
esac

systemctl restart "$SERVICE" && echo "$SERVICE restarted"
