#!/usr/bin/env bash
# Ships a git ref to the demo stand, builds it there and activates it:
#   infra/deploy.sh [ref]          (default: main)
# FSP_DEPLOY_HOST and FSP_SSH_CONFIG select the SSH target (defaults use .fsp-infra/).
set -euo pipefail

REF="${1:-main}"
DEPLOY_HOST="${FSP_DEPLOY_HOST:-fsp-demo}"
cd "$(git rev-parse --show-toplevel)"
SSH_CONFIG="${FSP_SSH_CONFIG:-.fsp-infra/ssh/config}"
SHA="$(git rev-parse --verify "$REF^{commit}")"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fsp-deploy.XXXXXX")"
ARCHIVE="$TEMP_DIR/release.tar.gz"
REMOTE_ARCHIVE="/tmp/fsp-$SHA-${ARCHIVE##*/}"
trap 'rm -f "$ARCHIVE"; rmdir "$TEMP_DIR"' EXIT

paths=(frontend mock-server backend infra)
# Historical releases may predate these optional runtime sources.
for path in contracts/openapi.json voice-runtime; do
  if git cat-file -e "$SHA:$path" 2>/dev/null; then
    paths+=("$path")
  fi
done
git archive --format=tar.gz -o "$ARCHIVE" "$SHA" "${paths[@]}"
scp -q -F "$SSH_CONFIG" "$ARCHIVE" "$DEPLOY_HOST:$REMOTE_ARCHIVE"
ssh -F "$SSH_CONFIG" "$DEPLOY_HOST" bash -s -- "$SHA" "$REMOTE_ARCHIVE" <<'REMOTE'
set -euo pipefail
sha="$1"
archive="$2"
root="${FSP_RELEASE_ROOT:-/opt/fsp}"
stage="$(mktemp -d "$root/releases/.staging-XXXXXXXX")"
trap 'rm -f "$archive"; rm -rf "$stage"' EXIT
chmod 755 "$stage"
tar -xzf "$archive" -C "$stage"
# Current deployment tooling never delegates to an unguarded legacy script, even
# if another deployment changes current between this check and lock acquisition.
if [[ "$(cat "$stage/backend/ROLLBACK_COMPATIBLE" 2>/dev/null)" != sqlx-forward-additive-v1 ]]; then
  echo 'Schema compatibility: target predates forward-migration support; use a compatible release' >&2
  exit 1
fi
# A historical archive may predate all release guards. Validate using the active
# controller before handing control to the archive's script (which takes the lock).
# This read-only early check is repeated under the lock by current release scripts.
if [[ -f "$root/current/infra/check-schema-compatibility.sh" ]]; then
  bash "$root/current/infra/check-schema-compatibility.sh" "$root/current" "$stage"
fi
if [[ -f "$root/current/infra/check-runtime-compatibility.sh" ]]; then
  bash "$root/current/infra/check-runtime-compatibility.sh" "$stage" "${FSP_SYSTEM_ROOT:-}/etc/fsp/rust-api.env"
fi
bash "$stage/infra/release.sh" "$sha" "$stage"
REMOTE
