#!/usr/bin/env bash
set -euo pipefail

TEMP="$(mktemp -d)"
trap 'rm -rf "$TEMP"' EXIT
curl -fsS -H 'Content-Type: application/json' \
  --data '{"text":"Проверяем распознавание русской речи.","voice":"eugene","sample_rate":24000}' \
  http://127.0.0.1:4050/synthesize > "$TEMP/control.wav"
ffmpeg -loglevel error -i "$TEMP/control.wav" -c:a libopus "$TEMP/control.webm"
# A non-seekable stream has no duration metadata, like browser MediaRecorder output.
ffmpeg -loglevel error -i "$TEMP/control.wav" -c:a libopus -f webm pipe:1 > "$TEMP/streaming.webm"
for recording in control streaming; do
  curl --max-time 45 -fsS -H 'Content-Type: audio/webm' --data-binary "@$TEMP/$recording.webm" \
    http://127.0.0.1:4060/transcribe > "$TEMP/$recording.json"
done
python3 - "$TEMP/control.json" "$TEMP/streaming.json" <<'PY'
import json
import sys

results = [json.load(open(path, encoding="utf-8")) for path in sys.argv[1:]]
for result in results:
    assert result["provider"] == "gigaam_v3_ctc"
    assert result["text"].strip()
    assert 0 < result["duration_seconds"] <= 25
    print(result["text"])
assert results[0]["text"] == results[1]["text"]
assert abs(results[0]["duration_seconds"] - results[1]["duration_seconds"]) < 0.02
PY
