#!/usr/bin/env bash
# A rollback must understand the active public hostname and authentication mode.
set -euo pipefail
target="${1:?usage: check-runtime-compatibility.sh <target-release> <rust-api.env>}"
config="${2:?missing runtime configuration}"
mode="$(sed -n 's/^FSP_AUTH_MODE=//p' "$config")"
host="$(sed -n 's|^FSP_PUBLIC_WEB_URL=https://||p' "$config")"
require_capability() {
  grep -Fxq "$1" "$target/infra/RUNTIME_CAPABILITIES" 2>/dev/null \
    || { echo "Runtime compatibility: target does not support active $1 configuration" >&2; exit 1; }
}
if [[ "${mode:-demo}" != demo ]]; then
  [[ "$mode" == resend ]] || { echo 'Invalid authentication mode' >&2; exit 1; }
  require_capability resend-auth
fi
if [[ -n "$host" && ! "$host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  require_capability domain-hostname
fi
# Stored data may use vocabulary an earlier release introduced (catalog-v2 grades, specializations and
# skills); every capability recorded next to the configuration must be understood by the target.
markers="$(dirname "$config")/data-capabilities"
if [[ -f "$markers" ]]; then
  while IFS= read -r capability; do
    if [[ -n "$capability" ]]; then require_capability "$capability"; fi
  done < "$markers"
fi
