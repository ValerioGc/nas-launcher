#!/usr/bin/env bash
# Applies validated application grants as a systemd unit drop-in.
# Reads fixed entries from /var/lib/nas-os/app-grants/<id> and rejects unknown values.
# Path grants expose approved host locations. Network grants enable SMTP or Syncthing
# through dedicated firewall sets. Background grants reduce application priority.
# Empty or removed requests restore the template defaults.
# Runs as root through nas-os-app-grants@<id>.service. Applications restart afterward
# to load the updated namespace and permissions.

set -euo pipefail

readonly REQUESTS=${NAS_GRANTS_DIR:-/var/lib/nas-os/app-grants}
readonly UNITS=${NAS_APP_DROPINS:-/etc/systemd/system}
# What the template hides with InaccessiblePaths, and a grant can show again.
readonly HIDDEN=(/sys /run/udev)

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

id=${1:-}
[[ $id =~ ^[A-Za-z0-9_-]{1,64}$ ]] || fail "not an application identifier: ${id:-<empty>}"

readonly request=$REQUESTS/$id
readonly dropin=$UNITS/nas-app@$id.service.d
readonly conf=$dropin/grants.conf

granted=()
if [[ -f $request ]]; then
  while IFS= read -r line || [[ -n $line ]]; do
    [[ -z $line ]] && continue
    case $line in
      /srv/storage | /proc | /sys | /run/udev | network:smtp | network:sync | run:background) granted+=("$line") ;;
      *) fail "$id asks for $line, which no application is given: nothing was changed" ;;
    esac
  done <"$request"
fi

is_granted() {
  local wanted=$1 path
  for path in "${granted[@]}"; do
    [[ $path == "$wanted" ]] && return 0
  done
  return 1
}

if ((${#granted[@]} == 0)); then
  rm -f -- "$conf"
  rmdir -- "$dropin" 2>/dev/null || true
  echo "$id: no grant, the template applies"
else
  text="# Written by apply-app-grants.sh for $id: what the launcher granted it. Do not edit by hand.
[Service]
"
  if is_granted /srv/storage; then
    text+="# The data disks, read and written in the group that owns them.
BindPaths=/srv/storage
SupplementaryGroups=nas-storage
Environment=NAS_STORAGE_ROOT=/srv/storage
Environment=NAS_STORAGE_REGISTRY=/etc/nas-os/storage-volumes.tsv
"
  fi
  if is_granted /proc; then
    text+="# Read the whole of /proc, including process metadata permitted by Unix ownership.
ProcSubset=all
ProtectProc=default
"
  fi
  if is_granted /sys || is_granted /run/udev; then
    still=()
    for path in "${HIDDEN[@]}"; do
      is_granted "$path" || still+=("-$path")
    done
    text+="# Read only, as the template keeps the kernel tunables.
InaccessiblePaths=
"
    ((${#still[@]} == 0)) || text+="InaccessiblePaths=${still[*]}
"
  fi
  if is_granted run:background; then
    text+="# Working without its window: behind what the person is doing on the NAS, never stopped.
Nice=10
CPUWeight=50
IOSchedulingClass=best-effort
IOSchedulingPriority=7
"
  fi
  if is_granted network:smtp; then
    text+="# Outgoing mail: the base firewall lets this set reach TCP 25, 465 and 587.
NFTSet=cgroup:inet:nas_base:nas_apps_smtp
"
  fi
  if is_granted network:sync; then
    text+="# The sync: the base firewall lets this set reach the local network on 22000 and 21027.
NFTSet=cgroup:inet:nas_base:nas_apps_sync
"
  fi
  install -d -m 0755 "$dropin"
  # Replaces the completed drop-in atomically.
  printf '%s' "$text" >"$conf.new"
  chmod 0644 "$conf.new"
  mv -f -- "$conf.new" "$conf"
  echo "$id: granted ${granted[*]}"
fi

systemctl daemon-reload
