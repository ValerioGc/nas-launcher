#!/usr/bin/env bash
#
# Shares on the local network the folders the applications asked for, with Samba:
#
#   apply-smb-shares.sh
#
# The launcher writes what each application shares, one folder per line, in
# /var/lib/nas-os/smb-shares/<id>: the name, the folder under /srv/storage, `ro` or `rw`, and
# `open` or `protected`, and the trash its deleted files go to or `-`, apart by tabs (a line of
# an older launcher has no trash). This script checks every line again and writes
# /etc/samba/smb.conf for them, after setting aside once what Debian installed there. Samba runs
# only while a folder is shared: with none, smbd is stopped and disabled. TCP 445 is open to the
# local network only meanwhile, through the set `nas_shares` of the base firewall.
#
# Every share is opened as the user of the account of the setup wizard, in the group that owns the
# data disks, so a file written over the network is the NAS's as any other. A protected share asks
# for that user and the password of the account, which the gateway hands to Samba; an open one lets
# anyone of the local network in. It runs as root from `nas-os-smb-shares.service`, which the
# launcher starts after a change and at its own start. To stop sharing everything by hand:
#
#   sudo rm /var/lib/nas-os/smb-shares/* && sudo systemctl start nas-os-smb-shares.service

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

# Whether Samba is the NAS's to run: its configuration is this script's, or the launcher installed
# it for an application. A Samba that was on the machine before is left alone while nothing is
# shared.
samba_is_ours() {
  { [[ -f $CONF ]] && head -n 1 "$CONF" | grep -qF -- "$MARK"; } ||
    { [[ -f $RECORD ]] && grep -qxF samba "$RECORD"; }
}

close_port() {
  nft flush set inet nas_base nas_shares >/dev/null 2>&1 || true
}

# A name Windows and Samba both read as it is: none of the characters they keep for themselves,
# no control character, no space at either end, and none of Samba's own sections.
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

# A folder below the data disks, as Samba reads it in its configuration: no `%`, no control
# character, no `.` or `..` and no segment Samba would trim.
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

# The trash of a shared folder: `-` for none, or the folder in the trash of its disk at the same
# path (`<disk>/.nas-trash/<folder>`, `<disk>/.nas-trash` for a whole disk).
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
# What Debian (or whoever) wrote stays aside, once: the removal of the launcher puts it back.
if [[ -f $CONF ]] && ! head -n 1 "$CONF" | grep -qF -- "$MARK" && [[ ! -e $ORIGINAL ]]; then
  cp -p -- "$CONF" "$ORIGINAL"
fi
chmod 0644 "$CONF.new"
mv -f -- "$CONF.new" "$CONF"

# NetBIOS is off: only smbd, on 445.
systemctl disable --now nmbd.service >/dev/null 2>&1 || true
systemctl enable smbd.service >/dev/null
systemctl reload-or-restart smbd.service
nft add element inet nas_base nas_shares "{ $PORT }" ||
  fail 'the base firewall has no set nas_shares: Samba runs, but port 445 stays closed (update the NAS)'
echo "${#shares[@]} folders shared on the local network, port 445 open to it"
