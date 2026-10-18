#!/usr/bin/env bash
# Applies validated NAS access rules through nftables.
# Reads normalized addresses or CIDR ranges from the launcher request file.
# Supports IPv4 and IPv6 entries.
# Runs as root through nas-os-firewall.service.

set -uo pipefail

readonly REQUEST=${NAS_ACCESS_RULES:-/var/lib/nas-os/access-rules}
readonly TABLE=nas_launcher
readonly CHAIN=input
readonly SET4=allowed_v4
readonly SET6=allowed_v6

fail() {
  printf 'access rules not applied: %s\n' "$1" >&2
  exit 1
}

command -v nft >/dev/null || fail 'nft is not installed (packages/os_configuration/install.sh)'
readonly CONNECTION=${NAS_GATEWAY_CONNECTION:-/var/lib/nas-os-gateway/gateway.connection}
if [[ -f $CONNECTION ]]; then
  IFS= read -r port <"$CONNECTION" || fail 'cannot read the HTTPS port'
  [[ $port =~ ^[1-9][0-9]{0,4}$ ]] && ((port <= 65535)) || fail 'invalid HTTPS port'
  printf 'flush set inet nas_base nas_https\nadd element inet nas_base nas_https { %s }\n' "$port" | nft -f - ||
    fail 'cannot update the HTTPS port: update the base firewall first'
fi
if [[ ! -r $REQUEST ]]; then
  [[ -f $CONNECTION ]] && exit 0
  fail "no list in $REQUEST"
fi

v4=()
v6=()
while IFS= read -r entry; do
  [[ -n $entry ]] || continue

  if [[ ! $entry =~ ^[0-9A-Fa-f:.]+(/[0-9]{1,3})?$ ]]; then
    printf 'skipped a line that is not an address\n' >&2
    continue
  fi
  if [[ $entry == *:* ]]; then
    v6+=("$entry")
  else
    v4+=("$entry")
  fi
done <"$REQUEST"

if ((${#v4[@]} == 0 && ${#v6[@]} == 0)); then
  nft list table inet "$TABLE" >/dev/null 2>&1 && nft delete table inet "$TABLE"
  printf 'access list empty: no restriction in place\n'
  exit 0
fi

join() { local IFS=,; printf '%s' "$*"; }

ruleset=$(
  cat <<RULES
table inet $TABLE {
  set $SET4 {
    type ipv4_addr
    flags interval
    auto-merge
    $( ((${#v4[@]})) && printf 'elements = { %s }' "$(join "${v4[@]}")")
  }

  set $SET6 {
    type ipv6_addr
    flags interval
    auto-merge
    $( ((${#v6[@]})) && printf 'elements = { %s }' "$(join "${v6[@]}")")
  }

  chain $CHAIN {
    type filter hook input priority filter; policy accept;

    iif lo accept
    ct state established,related accept

    udp sport 67 udp dport 68 accept
    udp sport 547 udp dport 546 accept
    icmp type { echo-request, destination-unreachable, time-exceeded } accept
    icmpv6 type { echo-request, nd-neighbor-solicit, nd-neighbor-advert, nd-router-advert, packet-too-big, destination-unreachable, time-exceeded } accept

    ip saddr @$SET4 accept
    ip6 saddr @$SET6 accept
    meta nfproto ipv4 drop
    meta nfproto ipv6 drop
  }
}
RULES
)

printf 'table inet %s {}\ndelete table inet %s\n%s\n' "$TABLE" "$TABLE" "$ruleset" | nft -f - ||
  fail 'nft refused the ruleset, the previous one is still in place'
printf 'access list applied: %d IPv4, %d IPv6\n' "${#v4[@]}" "${#v6[@]}"
