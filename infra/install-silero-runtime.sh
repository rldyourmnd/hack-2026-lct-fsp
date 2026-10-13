#!/usr/bin/env bash
set -euo pipefail

ROOT="${FSP_RELEASE_ROOT:-/opt/fsp}"
SOURCE="${1:-$ROOT/current/voice-runtime}"
VENV="$ROOT/voice-venv"
MODEL_DIR="$ROOT/models"
MODEL="$MODEL_DIR/v5_5_ru.pt"
MODEL_URL=https://models.silero.ai/models/tts/ru/v5_5_ru.pt
MODEL_SHA256=50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437

[[ -f "$SOURCE/requirements.lock" ]] || { echo "Missing $SOURCE/requirements.lock" >&2; exit 1; }
command -v python3.12 >/dev/null || { echo 'Python 3.12 is required' >&2; exit 1; }

if [[ ! -x "$VENV/bin/python" ]]; then
  python3.12 -m venv "$VENV"
fi
"$VENV/bin/pip" install --require-hashes \
  --index-url https://download.pytorch.org/whl/cpu \
  --requirement "$SOURCE/requirements.lock"

install -d -m 755 "$MODEL_DIR"
if [[ ! -f "$MODEL" ]] || ! echo "$MODEL_SHA256  $MODEL" | sha256sum -c - >/dev/null 2>&1; then
  temporary="$(mktemp "$MODEL_DIR/v5_5_ru.pt.XXXXXXXX")"
  trap 'rm -f "$temporary"' EXIT
  curl -fL --retry 3 --output "$temporary" "$MODEL_URL"
  echo "$MODEL_SHA256  $temporary" | sha256sum -c -
  chmod 644 "$temporary"
  mv "$temporary" "$MODEL"
  trap - EXIT
fi
echo "$MODEL_SHA256  $MODEL" | sha256sum -c -
