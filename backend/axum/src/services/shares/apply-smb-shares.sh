#!/usr/bin/env bash
# Applies validated application folder shares through Samba.
# Reads tab-separated records from /var/lib/nas-os/smb-shares/<id> and generates
# Samba configuration. Fields contain the share name, data-disk path, access mode,
# authentication mode, and optional recycle destination.
# Shares use the provisioned account and data-disk group. Protected shares require
# account credentials supplied by the gateway. Open shares allow local-network access.
# The helper preserves Debian's original configuration and enables smbd and TCP 445
# only while shares are active. Runs as root through nas-os-smb-shares.service.

set -euo pipefail

readonly REQUESTS=${NAS_SHARES_DIR:-/var/lib/nas-os/smb-shares}
readonly SAMBA_DIR=${NAS_SAMBA_DIR:-/etc/samba}
readonly CONF=$SAMBA_DIR/smb.conf
readonly ORIGINAL=$SAMBA_DIR/smb.conf.nas-os-original
readonly STORAGE=${NAS_STORAGE_ROOT:-/srv/storage}
readonly USER_CONF=${NAS_TERMINAL_DROPIN:-/etc/systemd/system/nas-os-terminal@.service.d}/user.conf
readonly RECORD=${NAS_APP_PACKAGES_RECORD:-/var/lib/nas-os-app-packages/installed}
readonly MARK='# Written by apply-smb-shares.sh'
readonly GROUP=nas-storage
readonly PORT=445

fail() {
  printf 'shared folders not applied: %s\n' "$1" >&2
  exit 1
}

# Checks whether the launcher owns Samba configuration or installed the package.
samba_is_ours() {
  { [[ -f $CONF ]] && head -n 1 "$CONF" | grep -qF -- "$MARK"; } ||
    { [[ -f $RECORD ]] && grep -qxF samba "$RECORD"; }
}

close_port() {
  nft flush set inet nas_base nas_shares >/dev/null 2>&1 || true
}

# Validates share names, excluding reserved names, special characters, and edge spaces.
valid_name() {
  local name=$1 lower
  [[ -n $name && ${#name} -le 256 ]] || return 1
  [[ $name != *[][/\\:\;\|=,+*?\<\>\"%]* && $name != *[[:cntrl:]]* ]] || return 1
  [[ $name != ' '* && $name != *' ' ]] || return 1
  lower=${name,,}
  case $lower in
    global | homes | printers | 'print$' | 'ipc$') return 1 ;;
  esac
}

# Validates literal Samba paths below data disks, excluding expansion and traversal.
valid_path() {
  local path=$1 rest segment
  local -a segments
  [[ $path == "$STORAGE"/* && ${#path} -le 4096 ]] || return 1
  [[ $path != *%* && $path != *[[:cntrl:]]* ]] || return 1
  rest=${path#"$STORAGE"/}
  [[ -n $rest && $rest != */ && $rest != *//* ]] || return 1
  IFS=/ read -r -a segments <<<"$rest"
  for segment in "${segments[@]}"; do
    [[ -n $segment && $segment != . && $segment != .. ]] || return 1
    [[ $segment != ' '* && $segment != *' ' ]] || return 1
  done
}

# Validates a recycle destination within the shared disk, or `-` when recycling is disabled.
valid_trash() {
  local trash=$1 path=$2 disk rest
  [[ $trash == - ]] && return 0
  valid_path "$trash" || return 1
  [[ $trash == */.nas-trash || $trash == */.nas-trash/* ]] || return 1
  disk=${trash%%/.nas-trash*}
  rest=${trash#"$disk"/.nas-trash}
  [[ $rest != *.nas-trash* ]] || return 1
  [[ $path == "$disk$rest" ]]
}

shares=()
seen=' '
shopt -s nullglob
for file in "$REQUESTS"/*; do
  [[ ${file##*/} =~ ^[A-Za-z0-9_-]{1,64}$ ]] || fail "${file##*/} is not the request of an application"
  while IFS= read -r line || [[ -n $line ]]; do
    [[ -z $line ]] && continue
    IFS=$'\t' read -r name path mode access trash extra <<<"$line"
    [[ -z ${extra:-} ]] || fail "${file##*/}: a line has more than five fields"
    trash=${trash:--}
    valid_name "$name" || fail "${file##*/}: $name is not a name a share can have"
    valid_path "$path" || fail "${file##*/}: $path is not a folder of the data disks"
    [[ $mode == ro || $mode == rw ]] || fail "${file##*/}: $mode is neither ro nor rw"
    [[ $access == open || $access == protected ]] || fail "${file##*/}: $access is neither open nor protected"
    valid_trash "$trash" "$path" || fail "${file##*/}: $trash is not the trash of $path"
    key=${name,,}
    [[ $seen != *" $key "* ]] || fail "$name is shared twice"
    seen+="$key "
    # A folder gone with its disk, or that a link would lead elsewhere, is left out until it is back.
    if [[ ! -d $path || $(realpath -e -- "$path" 2>/dev/null) != "$path" ]]; then
      echo "warning: $path is not there: $name is not shared for now"
      continue
    fi
    # The folder of a disk that is not mounted is an empty one on the system, not the disk.
    disk=${path#"$STORAGE"/}
    if ! mountpoint -q -- "$STORAGE/${disk%%/*}"; then
      echo "warning: the disk of $path is not mounted: $name is not shared for now"
      continue
    fi
    # Excludes network mounts from local Samba shares.
    case $(findmnt --noheadings --output FSTYPE --mountpoint "$STORAGE/${disk%%/*}" 2>/dev/null) in
      cifs | smb3 | nfs | nfs4 | fuse.sshfs)
        echo "warning: $path is on a network disk: $name is not shared"
        continue
        ;;
    esac
    shares+=("$name"$'\t'"$path"$'\t'"$mode"$'\t'"$access"$'\t'"$trash")
  done <"$file"
done

if ((${#shares[@]} == 0)); then
  close_port
  if command -v smbd >/dev/null && samba_is_ours; then
    systemctl disable --now smbd.service nmbd.service >/dev/null 2>&1 || true
    echo 'no folder shared: Samba stopped and port 445 closed'
  else
    echo 'no folder shared: port 445 closed'
  fi
  exit 0
fi

command -v smbd >/dev/null || fail 'samba is not installed'
command -v testparm >/dev/null || fail 'testparm is missing: samba is not installed completely'
user=$(sed -n 's/^User=//p' "$USER_CONF" 2>/dev/null | head -n 1) || user=''
[[ $user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] && getent passwd "$user" >/dev/null ||
  fail 'the account of the NAS has no user on the machine yet'
getent group "$GROUP" >/dev/null || fail "the group $GROUP is missing"

{
  printf '%s for the folders shared from the NAS. Do not edit by hand:\n' "$MARK"
  printf '# the next change of a share writes it again. What Debian installed is in %s.\n' "${ORIGINAL##*/}"
  cat <<'GLOBAL'

[global]
   server role = standalone server
   workgroup = WORKGROUP
   server string = NAS
   server min protocol = SMB2_10
   smb ports = 445
   disable netbios = yes
   map to guest = Bad User
   guest account = nobody
   passdb backend = tdbsam
   obey pam restrictions = no
   unix password sync = no
   load printers = no
   printing = bsd
   printcap name = /dev/null
   disable spoolss = yes
   follow symlinks = no
   wide links = no
   logging = file
   log file = /var/log/samba/log.%m
   max log size = 1000
GLOBAL
  for share in "${shares[@]}"; do
    IFS=$'\t' read -r name path mode access trash <<<"$share"
    printf '\n[%s]\n' "$name"
    printf '   path = %s\n' "$path"
    printf '   browseable = yes\n'
    printf '   read only = %s\n' "$([[ $mode == ro ]] && echo yes || echo no)"
    if [[ $access == protected ]]; then
      printf '   guest ok = no\n   valid users = %s\n' "$user"
    else
      printf '   guest ok = yes\n'
    fi
    printf '   force user = %s\n   force group = %s\n' "$user" "$GROUP"
    printf '   create mask = 0660\n   directory mask = 2770\n'
    # A file deleted from the share goes to the trash of its disk, with its tree, one version
    # for each path, as the Explorer puts it there.
    if [[ $trash != - ]]; then
      printf '   vfs objects = recycle\n   recycle:repository = %s\n' "$trash"
      printf '   recycle:keeptree = yes\n   recycle:versions = no\n   recycle:touch = yes\n'
      printf '   recycle:directory_mode = 2770\n   recycle:exclude_dir = .nas-trash\n'
    fi
  done
} >"$CONF.new"

if ! testparm -s "$CONF.new" >/dev/null 2>&1; then
  rm -f -- "$CONF.new"
  fail 'samba refused the configuration'
fi
# Preserves the original Samba configuration for restoration during uninstall.
if [[ -f $CONF ]] && ! head -n 1 "$CONF" | grep -qF -- "$MARK" && [[ ! -e $ORIGINAL ]]; then
  cp -p -- "$CONF" "$ORIGINAL"
fi
chmod 0644 "$CONF.new"
mv -f -- "$CONF.new" "$CONF"

# Uses SMB on TCP 445 without NetBIOS.
systemctl disable --now nmbd.service >/dev/null 2>&1 || true
systemctl enable smbd.service >/dev/null
systemctl reload-or-restart smbd.service
nft add element inet nas_base nas_shares "{ $PORT }" ||
  fail 'the base firewall has no set nas_shares: Samba runs, but port 445 stays closed (update the NAS)'
echo "${#shares[@]} folders shared on the local network, port 445 open to it"
