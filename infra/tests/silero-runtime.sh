#!/usr/bin/env bash
set -euo pipefail

ROOT="${FSP_RELEASE_ROOT:-/opt/fsp}"
PYTHON="${1:-$ROOT/voice-venv/bin/python}"
SERVER="${2:-$ROOT/current/voice-runtime/server.py}"
MODEL="${3:-$ROOT/models/v5_5_ru.pt}"
PORT="${SILERO_SMOKE_PORT:-14050}"
TEMP="$(mktemp -d)"
PID=
cleanup() {
  [[ -z "$PID" ]] || kill "$PID" 2>/dev/null || true
  rm -rf "$TEMP"
}
trap cleanup EXIT

SILERO_HOST=127.0.0.1 SILERO_PORT="$PORT" SILERO_MODEL_PATH="$MODEL" SILERO_CPU_THREADS=2 \
  "$PYTHON" "$SERVER" >"$TEMP/server.log" 2>&1 &
PID=$!
for _ in {1..120}; do
  if curl --max-time 2 -fsS "http://127.0.0.1:$PORT/health" >"$TEMP/health.json" 2>/dev/null; then
    break
  fi
  kill -0 "$PID" 2>/dev/null || { cat "$TEMP/server.log" >&2; exit 1; }
  sleep 0.25
done
curl --max-time 30 -fsS \
  -H 'Content-Type: application/json' \
  --data '{"text":"Здравствуйте. Начнём собеседование.","voice":"eugene","sample_rate":24000}' \
  "http://127.0.0.1:$PORT/synthesize" >"$TEMP/control.wav"
"$PYTHON" - "$TEMP/control.wav" <<'PY'
import sys
import wave

with wave.open(sys.argv[1], "rb") as audio:
    assert audio.getnchannels() == 1
    assert audio.getsampwidth() == 2
    assert audio.getframerate() == 24_000
    assert audio.getnframes() > 24_000
print("PASS: Silero v5.5 CPU runtime produced a valid Russian PCM16 WAV")
PY
