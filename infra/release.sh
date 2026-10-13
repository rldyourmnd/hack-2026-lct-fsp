#!/usr/bin/env bash
# Build an immutable release, then activate it with rollback on any activation failure.
set -Eeuo pipefail

SHA="${1:?usage: release.sh <sha> [staged-directory]}"
[[ "$SHA" =~ ^[0-9a-f]{40}$ ]] || { echo 'Expected a full commit SHA' >&2; exit 1; }
ROOT="${FSP_RELEASE_ROOT:-/opt/fsp}"
SYSTEM_ROOT="${FSP_SYSTEM_ROOT:-}"
CONFIG="$SYSTEM_ROOT/etc/fsp"
DIR="$ROOT/releases/$SHA"
STAGE="${2:-$DIR}"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
[[ -d "$STAGE" && ! -L "$STAGE" ]] || { echo 'Missing release directory' >&2; exit 1; }

# Shared by CI, manual deployments and rollbacks through build and health verification.
exec 9>"$ROOT/deploy.lock"
flock -w 1800 9
if [[ ! -f "$CONFIG/interview.env" ]]; then
  interview_env="$(mktemp "$CONFIG/interview.env.XXXXXXXX")"
  printf 'INTERVIEW_API_TOKEN=%s\n' "$(openssl rand -hex 32)" > "$interview_env"
  chown root:fsp "$interview_env"
  chmod 640 "$interview_env"
  mv "$interview_env" "$CONFIG/interview.env"
fi
IMPL="$(cat "$CONFIG/api-backend" 2>/dev/null || echo mock)"
case "$IMPL" in
  rust) active=fsp-api; inactive=fsp-mock-api ;;
  mock) active=fsp-mock-api; inactive=fsp-api ;;
  *) echo "Invalid api-backend: $IMPL" >&2; exit 1 ;;
esac

healthy() {
  curl --max-time 5 -fsS http://127.0.0.1:4010/health >/dev/null 2>&1 \
    && curl --max-time 5 -fsS --resolve "$(sed -n 's|^FSP_PUBLIC_WEB_URL=https://||p' "$CONFIG/rust-api.env"):443:127.0.0.1" -o /dev/null "$(sed -n 's|^FSP_PUBLIC_WEB_URL=||p' "$CONFIG/rust-api.env")/" || return 1
  if [[ "$(cat "$CONFIG/api-backend")" == rust && -x "$ROOT/current/bin/fsp-simulator" ]]; then
    curl --max-time 5 -fsS http://127.0.0.1:4020/health >/dev/null 2>&1 || return 1
  fi
  if [[ "$(cat "$CONFIG/api-backend")" == rust && -x "$ROOT/current/bin/fsp-github-insights" ]]; then
    curl --max-time 5 -fsS http://127.0.0.1:4030/health >/dev/null 2>&1 || return 1
  fi
  if [[ "$(cat "$CONFIG/api-backend")" == rust && -x "$ROOT/current/bin/fsp-interview-service" ]]; then
    curl --max-time 5 -fsS http://127.0.0.1:4040/health >/dev/null 2>&1 || return 1
  fi
  if grep -Fxq 'FSP_TTS_PROVIDER=silero' "$CONFIG/speech.env" 2>/dev/null \
    && [[ -f "$ROOT/current/voice-runtime/server.py" ]]; then
    curl --max-time 5 -fsS http://127.0.0.1:4050/health >/dev/null 2>&1 || return 1
  fi
  if grep -Fxq 'FSP_STT_PROVIDER=gigaam' "$CONFIG/speech.env" 2>/dev/null; then
    curl --max-time 5 -fsS http://127.0.0.1:4060/health >/dev/null 2>&1 || return 1
  fi
}

PREV="$(readlink -f "$ROOT/current" 2>/dev/null || true)"
if [[ "$PREV" == "$DIR" ]]; then
  healthy || { echo 'Current release is unhealthy; investigate or roll back' >&2; exit 1; }
  echo "Already active and healthy: $SHA"
  exit 0
fi

bash "$SCRIPTS/check-runtime-compatibility.sh" "$STAGE" "$CONFIG/rust-api.env"

if [[ ! -d "$DIR" ]]; then
  set -a
  # shellcheck disable=SC1091
  [[ ! -f "$CONFIG/frontend-build.env" ]] || . "$CONFIG/frontend-build.env"
  export VITE_RELEASE_SHA="$SHA"
  if [[ -f "$SYSTEM_ROOT/etc/nginx/fsp-observability/locations.conf" ]]; then
    export VITE_OBSERVABILITY=1
  fi
  set +a
  (cd "$STAGE/mock-server" && bun install --frozen-lockfile --production)
  (cd "$STAGE/frontend" && bun install --frozen-lockfile && bun run build && bun scripts/check-bundle.ts)

  # Select the pinned toolchain, never the first installed version.
  toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$STAGE/backend/rust-toolchain.toml")"
  [[ "$toolchain" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid Rust pin' >&2; exit 1; }
  rust_bin="${FSP_RUSTUP_ROOT:-/opt/rustup}/toolchains/$toolchain-x86_64-unknown-linux-gnu/bin"
  if [[ "$IMPL" == rust || -x "$rust_bin/cargo" ]]; then
    [[ -x "$rust_bin/cargo" ]] || { echo "Missing Rust $toolchain; run provisioning" >&2; exit 1; }
    (cd "$STAGE/backend" && PATH="$rust_bin:$PATH" RUSTUP_HOME="${FSP_RUSTUP_ROOT:-/opt/rustup}" \
      CARGO_TARGET_DIR="$ROOT/target-cache" "$rust_bin/cargo" build --release --locked \
        -p fsp-server -p fsp-integration-fsp-synthetic -p fsp-github-insights -p fsp-interview-service --bins)
    install -D -m 755 "$ROOT/target-cache/release/fsp-career-api" "$STAGE/bin/fsp-career-api"
    install -D -m 755 "$ROOT/target-cache/release/fsp-simulator" "$STAGE/bin/fsp-simulator"
    install -D -m 755 "$ROOT/target-cache/release/fsp-github-insights" "$STAGE/bin/fsp-github-insights"
    install -D -m 755 "$ROOT/target-cache/release/fsp-interview-service" "$STAGE/bin/fsp-interview-service"
  fi
  mv "$STAGE" "$DIR"
  if [[ "$SCRIPTS" == "$STAGE/infra" ]]; then SCRIPTS="$DIR/infra"; fi
fi
[[ -f "$DIR/frontend/dist/index.html" ]] || { echo 'Release has no frontend' >&2; exit 1; }
[[ "$IMPL" != rust || -x "$DIR/bin/fsp-career-api" ]] || { echo 'Release has no API binary' >&2; exit 1; }

# Validate before installing units, switching the symlink or changing the database.
if [[ "$IMPL" == rust && -n "$PREV" && -x "$PREV/bin/fsp-career-api" ]]; then
  bash "$SCRIPTS/check-schema-compatibility.sh" "$PREV" "$DIR"
fi
if [[ "$IMPL" == rust && "$(sed -n 's/^FSP_PDF_IMPORT=//p' "$CONFIG/rust-api.env")" == 1 ]]; then
  bash "$SCRIPTS/tests/pdf-runtime.sh" "$DIR/bin/fsp-career-api" "$DIR/infra/fsp-api.service"
fi

PREV_ACTIVE=fsp-mock-api
if systemctl is-active --quiet fsp-api; then PREV_ACTIVE=fsp-api; fi
backup="$(mktemp -d "$ROOT/.activation-XXXXXXXX")"
config_files=(
  etc/systemd/system/fsp-mock-api.service etc/systemd/system/fsp-api.service
  etc/systemd/system/fsp-simulator.service etc/systemd/system/fsp-github-insights.service
  etc/systemd/system/fsp-interview.service
  etc/systemd/system/fsp-silero-tts.service
  etc/systemd/system/fsp-gigaam-stt.service
  etc/systemd/system/fsp-cliproxy.service etc/systemd/system/fsp-bifrost.service
  etc/nginx/sites-available/fsp-demo.conf etc/nginx/snippets/fsp-security-headers.conf
)
for file in "${config_files[@]}"; do
  if [[ -f "$SYSTEM_ROOT/$file" ]]; then
    install -D -m 644 "$SYSTEM_ROOT/$file" "$backup/$file"
  fi
done

switch_link() {
  ln -sfn "$1" "$ROOT/current.next"
  mv -Tf "$ROOT/current.next" "$ROOT/current"
}

rollback() {
  local status="$?"
  trap - ERR HUP INT TERM
  set +e
  echo "Activation failed for $SHA; restoring previous release" >&2
  for file in "${config_files[@]}"; do
    if [[ -f "$backup/$file" ]]; then
      install -D -m 644 "$backup/$file" "$SYSTEM_ROOT/$file"
    else
      rm -f "$SYSTEM_ROOT/$file"
    fi
  done
  if [[ -n "$PREV" && -d "$PREV" ]]; then
    switch_link "$PREV"
    systemctl daemon-reload
    if [[ "$PREV_ACTIVE" == fsp-api && -x "$PREV/bin/fsp-simulator" ]]; then
      systemctl enable fsp-simulator
      systemctl restart fsp-simulator
    else
      systemctl disable --now fsp-simulator 2>/dev/null || true
    fi
    if [[ "$PREV_ACTIVE" == fsp-api && -x "$PREV/bin/fsp-github-insights" ]]; then
      systemctl enable fsp-github-insights
      systemctl restart fsp-github-insights
    else
      systemctl disable --now fsp-github-insights 2>/dev/null || true
    fi
    if [[ "$PREV_ACTIVE" == fsp-api && -x "$PREV/bin/fsp-interview-service" ]]; then
      systemctl enable fsp-interview
      systemctl restart fsp-interview
    else
      systemctl disable --now fsp-interview 2>/dev/null || true
    fi
    if grep -Fxq 'FSP_TTS_PROVIDER=silero' "$CONFIG/speech.env" 2>/dev/null \
      && [[ -f "$PREV/voice-runtime/server.py" ]]; then
      systemctl enable fsp-silero-tts
      systemctl restart fsp-silero-tts
    else
      systemctl disable --now fsp-silero-tts 2>/dev/null || true
    fi
    if grep -Fxq 'FSP_STT_PROVIDER=gigaam' "$CONFIG/speech.env" 2>/dev/null \
      && [[ -f "$PREV/voice-runtime/stt_server.py" ]]; then
      systemctl enable fsp-gigaam-stt
      systemctl restart fsp-gigaam-stt
    else
      systemctl disable --now fsp-gigaam-stt 2>/dev/null || true
    fi
    if [[ "$PREV_ACTIVE" == fsp-api ]]; then
      echo rust > "$CONFIG/api-backend"
      systemctl disable --now fsp-mock-api
    else
      echo mock > "$CONFIG/api-backend"
      systemctl disable --now fsp-api
    fi
    systemctl enable "$PREV_ACTIVE"
    systemctl restart "$PREV_ACTIVE"
    nginx -t && systemctl reload nginx
    if healthy; then echo "Restored ${PREV##*/}" >&2; else echo 'Rollback health check failed' >&2; fi
  else
    systemctl stop "$active"
    systemctl disable --now fsp-simulator
    systemctl disable --now fsp-github-insights 2>/dev/null || true
    systemctl disable --now fsp-interview 2>/dev/null || true
    systemctl disable --now fsp-silero-tts 2>/dev/null || true
    systemctl disable --now fsp-gigaam-stt 2>/dev/null || true
    rm -f "$ROOT/current"
    echo 'No previous release available' >&2
  fi
  rm -rf "$backup"
  exit "$((status == 0 ? 1 : status))"
}
trap rollback ERR HUP INT TERM

for unit in fsp-api fsp-mock-api fsp-cliproxy fsp-bifrost; do
  install -m 644 "$DIR/infra/$unit.service" "$SYSTEM_ROOT/etc/systemd/system/$unit.service"
done
# Helper absence is supported only after the schema compatibility guard passes.
if [[ -f "$DIR/infra/fsp-simulator.service" ]]; then
  install -m 644 "$DIR/infra/fsp-simulator.service" "$SYSTEM_ROOT/etc/systemd/system/fsp-simulator.service"
fi
if [[ -f "$DIR/infra/fsp-github-insights.service" ]]; then
  install -m 644 "$DIR/infra/fsp-github-insights.service" "$SYSTEM_ROOT/etc/systemd/system/fsp-github-insights.service"
fi
if [[ -f "$DIR/infra/fsp-interview.service" ]]; then
  install -m 644 "$DIR/infra/fsp-interview.service" "$SYSTEM_ROOT/etc/systemd/system/fsp-interview.service"
fi
install -m 644 "$DIR/infra/fsp-silero-tts.service" "$SYSTEM_ROOT/etc/systemd/system/fsp-silero-tts.service"
install -m 644 "$DIR/infra/fsp-gigaam-stt.service" "$SYSTEM_ROOT/etc/systemd/system/fsp-gigaam-stt.service"
bash "$DIR/infra/render-nginx.sh" "$CONFIG/rust-api.env" > "$SYSTEM_ROOT/etc/nginx/sites-available/fsp-demo.conf"
install -m 644 "$DIR/infra/nginx/fsp-security-headers.conf" "$SYSTEM_ROOT/etc/nginx/snippets/fsp-security-headers.conf"
nginx -t
switch_link "$DIR"
systemctl daemon-reload
if [[ "$IMPL" == rust && -x "$DIR/bin/fsp-simulator" ]]; then
  systemctl enable fsp-simulator
  systemctl restart fsp-simulator
else
  systemctl disable --now fsp-simulator
fi
if [[ "$IMPL" == rust && -x "$DIR/bin/fsp-github-insights" ]]; then
  systemctl enable fsp-github-insights
  systemctl restart fsp-github-insights
else
  # Releases before the insights service have no unit to disable.
  systemctl disable --now fsp-github-insights 2>/dev/null || true
fi
if [[ "$IMPL" == rust && -x "$DIR/bin/fsp-interview-service" ]]; then
  systemctl enable fsp-interview
  systemctl restart fsp-interview
else
  systemctl disable --now fsp-interview 2>/dev/null || true
fi
if [[ "$IMPL" == rust ]] && grep -Fxq 'FSP_TTS_PROVIDER=silero' "$CONFIG/speech.env" 2>/dev/null; then
  [[ -x "$ROOT/voice-venv/bin/python" && -r "$ROOT/models/v5_5_ru.pt" ]] \
    || { echo 'Silero runtime is missing; run infra/install-silero-runtime.sh' >&2; exit 1; }
  systemctl enable fsp-silero-tts
  systemctl restart fsp-silero-tts
else
  systemctl disable --now fsp-silero-tts 2>/dev/null || true
fi
if [[ "$IMPL" == rust ]] && grep -Fxq 'FSP_STT_PROVIDER=gigaam' "$CONFIG/speech.env" 2>/dev/null; then
  [[ -x "$ROOT/voice-venv/bin/python" && -r "$ROOT/models/gigaam/v3_ctc.ckpt" ]] \
    || { echo 'GigaAM runtime is missing; run infra/install-gigaam-runtime.sh' >&2; exit 1; }
  systemctl enable fsp-gigaam-stt
  systemctl restart fsp-gigaam-stt
else
  systemctl disable --now fsp-gigaam-stt 2>/dev/null || true
fi
systemctl disable --now "$inactive"
systemctl enable "$active"
systemctl restart "$active"
systemctl reload nginx

ready=0
for _ in {1..60}; do
  if healthy; then ready=1; break; fi
  sleep 0.5
done
[[ "$ready" == 1 ]]
trap - ERR HUP INT TERM
# Data this release writes may use catalog-v2 values older releases cannot read: record it once, so
# later rollbacks to releases without the capability are refused.
if grep -Fxq catalog-v2 "$DIR/infra/RUNTIME_CAPABILITIES" 2>/dev/null \
  && ! grep -Fxqs catalog-v2 "$CONFIG/data-capabilities"; then
  echo catalog-v2 >> "$CONFIG/data-capabilities"
fi
rm -rf "$backup"

# Keep the current and previous releases even when rolling back to an older SHA.
kept=0
while IFS= read -r release; do
  [[ "${release##*/}" =~ ^[0-9a-f]{40}$ ]] || continue
  kept=$((kept + 1))
  if [[ "$kept" -gt 5 && "$release" != "$DIR" && "$release" != "$PREV" ]]; then
    rm -rf "$release"
  fi
done < <(find "$ROOT/releases" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' | sort -nr | cut -d ' ' -f 2-)
echo "Active release: $SHA (api: $IMPL)"
