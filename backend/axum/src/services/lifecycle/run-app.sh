#!/usr/bin/env bash
#
# Runs an installed application. `nas-app@<id>.service` calls it with the identifier 
#
#   run-app.sh <id>
#
# Reads `run.exec` from /var/lib/nas-os/apps/<id>/nas-app.json and runs that program, handing it
# the socket the launcher will connect to.

set -euo pipefail

readonly APPS_DIR=${NAS_APPS_DIR:-/var/lib/nas-os/apps}
readonly SOCKETS_DIR=${NAS_APP_SOCKETS:-/run/nas-os/apps}

id=${1:-}
if [[ ! $id =~ ^[A-Za-z0-9_-]{1,64}$ ]]; then
  printf 'not an application identifier: %s\n' "${id:-<empty>}" >&2
  exit 2
fi

readonly manifest=$APPS_DIR/$id/nas-app.json
if [[ ! -f $manifest ]]; then
  printf 'no manifest in %s: nothing to run\n' "$APPS_DIR/$id" >&2
  exit 3
fi

# The `exec` value taken from the manifest, without a JSON parser on the machine.
declared=$(sed -n 's/.*"exec"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$manifest" | head -1)
if [[ -z $declared ]]; then
  printf '%s declares no run.exec\n' "$manifest" >&2
  exit 3
fi
if [[ $declared == /* || $declared == *..* ]]; then
  printf 'run.exec has to stay inside the package: %s\n' "$declared" >&2
  exit 4
fi

readonly program=$APPS_DIR/$id/$declared
if [[ ! -x $program ]]; then
  printf '%s is not executable\n' "$program" >&2
  exit 4
fi

readonly socket=$SOCKETS_DIR/$id.sock

rm -f "$socket"

export NAS_APP_ID=$id
export NAS_APP_SOCKET=$socket
exec "$program" --listen "$socket"
