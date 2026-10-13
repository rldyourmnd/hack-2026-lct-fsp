#!/usr/bin/env bash
# Probe the actual proxy: overload must be immediate, structured and retryable.
set -euo pipefail
base="${1:?usage: nginx.sh <base-url>}"
directory="$(mktemp -d)"
trap 'rm -rf "$directory"' EXIT
curl --max-time 3 -fsS -D "$directory/headers" -o "$directory/index.html" "$base/"
grep -Eqi '^Permissions-Policy:.*camera=\(\), microphone=\(self\), geolocation=\(\)' "$directory/headers"
grep -Eqi "^Content-Security-Policy:.*media-src 'self' blob:;" "$directory/headers"
for path in / /api/v1/meta /api/v1/missing-header-probe /assets/missing-header-probe.js /__mock__/reset; do
  curl --max-time 3 -sS -D "$directory/headers" -o /dev/null "$base$path"
  [[ "$(grep -ci '^X-Content-Type-Options:' "$directory/headers")" == 1 ]]
  grep -Eqi '^X-Content-Type-Options: nosniff[[:space:]]*$' "$directory/headers"
  if [[ "$base" == https://* ]]; then
    grep -Eqi '^Strict-Transport-Security: max-age=604800[[:space:]]*$' "$directory/headers"
  else
    if grep -qi '^Strict-Transport-Security:' "$directory/headers"; then
      echo 'Plain HTTP must not advertise HSTS' >&2
      exit 1
    fi
  fi
  if [[ "$path" == /api/v1/* || "$path" == /assets/missing-header-probe.js ]]; then
    [[ "$(grep -ci '^Cache-Control:' "$directory/headers")" == 1 ]]
    grep -Eqi '^Cache-Control: no-store[[:space:]]*$' "$directory/headers"
  fi
done
# Existing content-addressed assets remain cacheable for complete, partial and conditional reads.
asset="$(sed -n 's/.*src="\(\/assets\/[^" ]*\.js\)".*/\1/p' "$directory/index.html")"
[[ -n "$asset" && "$asset" != *$'\n'* ]]
status="$(curl --max-time 3 -fsS -D "$directory/headers" -o /dev/null -w '%{http_code}' "$base$asset")"
[[ "$status" == 200 ]]
grep -Eqi '^Cache-Control: public, max-age=31536000, immutable[[:space:]]*$' "$directory/headers"
etag="$(awk 'tolower($1) == "etag:" {gsub("\r", "", $2); print $2}' "$directory/headers")"
[[ -n "$etag" ]]
status="$(curl --max-time 3 -fsS -H 'Range: bytes=0-0' -D "$directory/headers" -o /dev/null -w '%{http_code}' "$base$asset")"
[[ "$status" == 206 ]]
grep -Eqi '^Cache-Control: public, max-age=31536000, immutable[[:space:]]*$' "$directory/headers"
status="$(curl --max-time 3 -fsS -H "If-None-Match: $etag" -D "$directory/headers" -o /dev/null -w '%{http_code}' "$base$asset")"
[[ "$status" == 304 ]]
grep -Eqi '^Cache-Control: public, max-age=31536000, immutable[[:space:]]*$' "$directory/headers"
limited=0
for _ in {1..13}; do
  status="$(curl --max-time 3 -sS -D "$directory/headers" -o "$directory/body" -w '%{http_code}' \
    -X POST -H 'Content-Type: application/json' -d '{}' "$base/api/v1/auth/email/challenges")"
  if [[ "$status" == 429 ]]; then
    jq -e '.code == "rate_limited" and (.requestId | length > 0) and .details == []' "$directory/body" >/dev/null
    grep -qi '^Retry-After: 60' "$directory/headers"
    grep -qi '^Cache-Control: no-store' "$directory/headers"
    limited=1
  fi
done
[[ "$limited" == 1 ]]
# Non-authentication reads do not spend or inherit the anonymous auth budget.
[[ "$(curl --max-time 3 -sS -o /dev/null -w '%{http_code}' "$base/api/v1/meta")" != 429 ]]
echo 'nginx: unique security/cache headers, HTTPS-only HSTS, structured scoped rate limit'
