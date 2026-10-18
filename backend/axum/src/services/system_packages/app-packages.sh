#!/usr/bin/env bash
# Installs allowed Debian dependencies and removes unused launcher-managed packages.
# Reads one package per line from /var/lib/nas-os/app-packages/<id>.
# Rejects entries outside the fixed allowlist and preserves pre-existing packages.
# Runs as root through nas-os-app-packages@<id>.service.

set -euo pipefail

readonly REQUESTS=${NAS_APP_PACKAGES_DIR:-/var/lib/nas-os/app-packages}
readonly RECORD=${NAS_APP_PACKAGES_RECORD:-/var/lib/nas-os-app-packages/installed}
readonly INSTALLABLE=(msmtp clamav clamav-freshclam 7zip samba syncthing cifs-utils)
export DEBIAN_FRONTEND=noninteractive
# Waits for other apt or dpkg jobs to release the package-manager lock.
readonly LOCK_WAIT=(-o DPkg::Lock::Timeout=300)

fail() {
  printf 'system packages not set up: %s\n' "$1" >&2
  exit 1
}

id=${1:-}
[[ $id =~ ^[A-Za-z0-9_-]{1,64}$ ]] || fail "not an application identifier: ${id:-<empty>}"
readonly RESTORE=${NAS_CONFIGURATION_HELPER:-/usr/local/lib/nas-wizard/packages/os_configuration/scripts/uninstall.sh}
[[ -x $RESTORE ]] || fail 'the system restoration helper is missing'
"$RESTORE" --check
before=$(mktemp)
trap 'rm -f -- "$before"' EXIT
dpkg-query -W -f='${binary:Package}\t${db:Status-Status}\n' |
  awk '$2 == "installed" { print $1 }' | LC_ALL=C sort -u >"$before"
record_packages() {
  local status=$?
  "$RESTORE" --record-packages "$before" || status=1
  rm -f -- "$before"
  exit "$status"
}
trap record_packages EXIT

is_installable() {
  local name
  for name in "${INSTALLABLE[@]}"; do
    [[ $name == "$1" ]] && return 0
  done
  return 1
}

# Reads into `names` the packages a request file lists, each checked against the list above.
read_request() {
  names=()
  local line
  while IFS= read -r line || [[ -n $line ]]; do
    [[ -z $line ]] && continue
    is_installable "$line" || fail "${1##*/} asks for $line, which no application is given"
    names+=("$line")
  done <"$1"
}

is_present() {
  [[ $(dpkg-query -W -f='${Status}' "$1" 2>/dev/null) == 'install ok installed' ]]
}

mkdir -p "$(dirname "$RECORD")"
touch "$RECORD"
# Serializes access to the managed-package register.
exec 9>"$RECORD.lock"
flock 9

if [[ -f $REQUESTS/$id ]]; then
  read_request "$REQUESTS/$id"
  for name in "${names[@]}"; do
    is_present "$name" && continue
    grep -qxF -- "$name" "$RECORD" || printf '%s\n' "$name" >>"$RECORD"
    if [[ $id == nas-security-center ]]; then
      progress="$(dirname "$RECORD")/antivirus-install.progress"
      rm -f -- "$progress"
      install -m 0644 -o root -g root /dev/null "$progress"
      apt-get install -q -y --no-install-recommends "${LOCK_WAIT[@]}" -o APT::Status-Fd=3 -- "$name" 3>"$progress" || fail "apt-get could not install $name"
    else
      apt-get install -q -y --no-install-recommends "${LOCK_WAIT[@]}" -- "$name" || fail "apt-get could not install $name"
    fi
    echo "$id: installed $name"
  done
fi

still=()
shopt -s nullglob
for file in "$REQUESTS"/*; do
  read_request "$file"
  still+=("${names[@]}")
done

kept=()
while IFS= read -r name; do
  [[ -z $name ]] && continue
  if printf '%s\n' "${still[@]}" | grep -qxF -- "$name"; then
    kept+=("$name")
  elif is_present "$name"; then
    apt-get purge -q -y "${LOCK_WAIT[@]}" -- "$name" || fail "apt-get could not remove $name"
    echo "removed $name, which no application needs any more"
  fi
done <"$RECORD"

printf '%s\n' "${kept[@]}" | grep -v '^$' >"$RECORD.new" || true
mv -f -- "$RECORD.new" "$RECORD"
