#!/usr/bin/env bash
# Stores private-release tokens as encrypted systemd credentials.
# Uses the machine key and TPM when available. The launcher receives decrypted
# credentials only when its service starts.
# Accepts interactive input or standard input and supports clearing the credential.
# Runs during configuration or through nas-os-token.service.

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
