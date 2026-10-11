#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf 'FSP_PUBLIC_WEB_URL=https://192.0.2.1\n' > "$tmp/env"
bash "$here/render-nginx.sh" "$tmp/env" > "$tmp/nginx"
grep -q 'server_name 192.0.2.1;' "$tmp/nginx"
grep -q 'live/fsp-demo/fullchain.pem' "$tmp/nginx"
printf 'FSP_PUBLIC_WEB_URL=https://nddev.link\nFSP_TLS_CERT_NAME=nddev.link\n' > "$tmp/env"
bash "$here/render-nginx.sh" "$tmp/env" > "$tmp/nginx"
grep -q 'live/nddev.link/fullchain.pem' "$tmp/nginx"
grep -q "return 308 https://nddev.link\$request_uri;" "$tmp/nginx"
grep -q 'real_ip_header CF-Connecting-IP;' "$tmp/nginx"
grep -q 'Cache-Control "no-store"' "$tmp/nginx"
for bad in 'https://example.com/path' 'http://example.com' 'https://example.com;return 200;' 'https://../etc'; do
  printf 'FSP_PUBLIC_WEB_URL=%s\n' "$bad" > "$tmp/env"
  if bash "$here/render-nginx.sh" "$tmp/env" > /dev/null 2>&1; then echo "Accepted invalid host: $bad" >&2; exit 1; fi
done
echo 'PASS: IP/domain certificates, canonical redirect, trusted proxy and invalid configuration'

printf 'FSP_AUTH_MODE=resend\nFSP_PUBLIC_WEB_URL=https://nddev.link\n' > "$tmp/env"
bash "$here/check-runtime-compatibility.sh" "$here/.." "$tmp/env"
mkdir -p "$tmp/legacy/infra"
if bash "$here/check-runtime-compatibility.sh" "$tmp/legacy" "$tmp/env" > /dev/null 2>&1; then
  echo 'Legacy rollback accepted real-mail runtime' >&2; exit 1
fi
echo 'PASS: rollback rejects releases without real-mail/domain support'

printf 'FSP_AUTH_MODE=demo\n' > "$tmp/env"
printf 'catalog-v2\n' > "$tmp/data-capabilities"
bash "$here/check-runtime-compatibility.sh" "$here/.." "$tmp/env"
mkdir -p "$tmp/pre-catalog/infra"
printf 'resend-auth\ndomain-hostname\n' > "$tmp/pre-catalog/infra/RUNTIME_CAPABILITIES"
if bash "$here/check-runtime-compatibility.sh" "$tmp/pre-catalog" "$tmp/env" > /dev/null 2>&1; then
  echo 'Rollback below catalog-v2 accepted stored catalog-v2 data' >&2; exit 1
fi
echo 'PASS: rollback rejects releases that cannot read catalog-v2 data'
