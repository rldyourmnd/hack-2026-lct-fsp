#!/usr/bin/env bash
set -euo pipefail

ROOT="${FSP_RELEASE_ROOT:-/opt/fsp}"
VENV="$ROOT/voice-venv"
SOURCE_DIR="$ROOT/gigaam-source"
MODEL_DIR="$ROOT/models/gigaam"
COMMIT=7447938d791c4f3e643386ee22c33777004293a5
SOURCE_URL="https://github.com/salute-developers/GigaAM/archive/$COMMIT.tar.gz"
SOURCE_SHA256=6ff116580aebee5f4b32d62ed7938e30004815caf85fcbbc301d5b9b3f001d3f
MODEL_URL=https://cdn.chatwm.opensmodel.sberdevices.ru/GigaAM/v3_ctc.ckpt
MODEL_MD5=73413e7be9c6a5935827bfab5c0dd678

[[ -x "$VENV/bin/python" ]] || { echo 'Install the Silero voice runtime first' >&2; exit 1; }
command -v ffmpeg >/dev/null || { echo 'ffmpeg is required' >&2; exit 1; }
command -v ffprobe >/dev/null || { echo 'ffprobe is required' >&2; exit 1; }
"$VENV/bin/pip" install numpy==2.2.6 hydra-core==1.3.2 omegaconf==2.3.0 tqdm==4.67.1 soundfile==0.13.1 sentencepiece==0.2.1
"$VENV/bin/pip" install --index-url https://download.pytorch.org/whl/cpu torchaudio==2.5.1+cpu

archive="$(mktemp "$ROOT/gigaam-source.XXXXXXXX.tar.gz")"
trap 'rm -f "$archive"' EXIT
curl -fL --retry 3 --output "$archive" "$SOURCE_URL"
echo "$SOURCE_SHA256  $archive" | sha256sum -c -
rm -rf "$SOURCE_DIR"
mkdir -p "$SOURCE_DIR"
tar -xzf "$archive" --strip-components=1 -C "$SOURCE_DIR"
"$VENV/bin/pip" install --no-deps "$SOURCE_DIR"

install -d -m 755 "$MODEL_DIR"
model="$MODEL_DIR/v3_ctc.ckpt"
if [[ ! -f "$model" ]] || ! echo "$MODEL_MD5  $model" | md5sum -c - >/dev/null 2>&1; then
  temporary="$(mktemp "$MODEL_DIR/v3_ctc.ckpt.XXXXXXXX")"
  curl -fL --retry 3 --output "$temporary" "$MODEL_URL"
  echo "$MODEL_MD5  $temporary" | md5sum -c -
  chmod 644 "$temporary"
  mv "$temporary" "$model"
fi
echo "$MODEL_MD5  $model" | md5sum -c -
