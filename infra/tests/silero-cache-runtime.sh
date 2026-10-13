#!/usr/bin/env bash
set -euo pipefail

ROOT="${FSP_RELEASE_ROOT:-/opt/fsp}"
PYTHON="${1:-$ROOT/voice-venv/bin/python}"
SERVER="${2:-$ROOT/current/voice-runtime/server.py}"
MODEL="${3:-$ROOT/models/v5_5_ru.pt}"
PORT="${SILERO_CACHE_SMOKE_PORT:-14051}"
TEMP="$(mktemp -d)"
PID=
cleanup() {
  [[ -z "$PID" ]] || kill "$PID" 2>/dev/null || true
  rm -rf "$TEMP"
}
trap cleanup EXIT

start_server() {
  SILERO_HOST=127.0.0.1 SILERO_PORT="$PORT" SILERO_MODEL_PATH="$MODEL" \
    SILERO_CPU_THREADS=2 SILERO_CACHE_DIR="$TEMP/cache" \
    "$PYTHON" "$SERVER" >"$TEMP/server.log" 2>&1 &
  PID=$!
  for _ in {1..120}; do
    if curl --max-time 2 -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
      return
    fi
    kill -0 "$PID" 2>/dev/null || { cat "$TEMP/server.log" >&2; exit 1; }
    sleep 0.25
  done
  echo 'Silero cache smoke server did not become healthy' >&2
  exit 1
}

request() {
  local output=$1
  curl --max-time 30 -fsS -o "$output" -w '%{time_total}' \
    -H 'Content-Type: application/json' \
    --data "{\"text\":\"Проверка постоянного кеша ${BASHPID}\",\"voice\":\"eugene\",\"sample_rate\":24000}" \
    "http://127.0.0.1:$PORT/synthesize"
}

start_server
cold_seconds="$(request "$TEMP/cold.wav")"
warm_seconds="$(request "$TEMP/warm.wav")"
cmp "$TEMP/cold.wav" "$TEMP/warm.wav"
kill "$PID"
wait "$PID" 2>/dev/null || true
PID=

start_server
restart_seconds="$(request "$TEMP/restart.wav")"
cmp "$TEMP/cold.wav" "$TEMP/restart.wav"

"$PYTHON" - "$cold_seconds" "$warm_seconds" "$restart_seconds" <<'PY'
import json
import sys

cold, warm, restart = map(float, sys.argv[1:])
assert warm < cold / 2, (cold, warm)
assert restart < cold / 2, (cold, restart)
print(json.dumps({
    "status": "passed",
    "coldSeconds": round(cold, 4),
    "warmSeconds": round(warm, 4),
    "postRestartSeconds": round(restart, 4),
    "byteIdentical": True,
}))
PY
