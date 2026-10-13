#!/usr/bin/env python3

import io
import json
import os
import subprocess
import tempfile
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


MAX_AUDIO_BYTES = 4 * 1024 * 1024
MAX_RECORDING_SECONDS = 25
SAMPLE_RATE = 16_000
SUFFIXES = {"audio/webm": ".webm", "audio/mp4": ".mp4", "audio/ogg": ".ogg"}


def decode_recording(source: str) -> tuple[bytes, float]:
    # Streaming browser containers may omit duration. Count decoded samples instead.
    pcm = subprocess.run(
        [
            "ffmpeg", "-nostdin", "-v", "error", "-protocol_whitelist", "file,pipe",
            "-i", source, "-map", "0:a:0", "-vn",
            "-t", str(MAX_RECORDING_SECONDS + 1),
            "-ac", "1", "-ar", str(SAMPLE_RATE), "-f", "s16le", "pipe:1",
        ],
        capture_output=True,
        check=True,
        timeout=10,
    ).stdout
    duration = len(pcm) / (SAMPLE_RATE * 2)
    if not pcm or len(pcm) % 2 or duration > MAX_RECORDING_SECONDS:
        raise ValueError("recording must be between 0 and 25 seconds")
    output = io.BytesIO()
    with wave.open(output, "wb") as recording:
        recording.setnchannels(1)
        recording.setsampwidth(2)
        recording.setframerate(SAMPLE_RATE)
        recording.writeframes(pcm)
    return output.getvalue(), duration


class GigaAmRuntime:
    def __init__(self, model_dir: Path, threads: int) -> None:
        import gigaam
        import torch

        torch.set_num_threads(threads)
        torch.set_num_interop_threads(1)
        self._model = gigaam.load_model(
            "v3_ctc", fp16_encoder=False, device="cpu", download_root=str(model_dir)
        )
        self._lock = threading.Lock()

    def transcribe(self, audio: bytes, suffix: str) -> tuple[str, float]:
        with tempfile.NamedTemporaryFile(suffix=suffix) as source:
            source.write(audio)
            source.flush()
            wav, duration = decode_recording(source.name)
        with tempfile.NamedTemporaryFile(suffix=".wav") as normalized:
            normalized.write(wav)
            normalized.flush()
            with self._lock:
                result = self._model.transcribe(normalized.name)
        text = result.text.strip()
        if not text:
            raise ValueError("speech was not recognized")
        return text, duration


def handler(runtime: GigaAmRuntime):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            if self.path != "/health":
                self.send_error(404)
                return
            self._respond(200, b'{"status":"ok","model":"gigaam_v3_ctc","device":"cpu"}')

        def do_POST(self) -> None:
            if self.path != "/transcribe":
                self.send_error(404)
                return
            content_type = self.headers.get("Content-Type", "").split(";", 1)[0]
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if content_type not in SUFFIXES or length < 1 or length > MAX_AUDIO_BYTES:
                    raise ValueError("invalid recording")
                text, duration = runtime.transcribe(self.rfile.read(length), SUFFIXES[content_type])
            except (ValueError, subprocess.SubprocessError) as error:
                self._respond(422, json.dumps({"error": str(error)}).encode())
                return
            body = json.dumps(
                {"text": text, "provider": "gigaam_v3_ctc", "duration_seconds": duration},
                ensure_ascii=False,
            ).encode()
            self._respond(200, body)

        def _respond(self, status: int, body: bytes) -> None:
            try:
                self.send_response(status)
                self.send_header("Content-Type", "application/json; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                self.close_connection = True
                self.log_message("client disconnected before response completed")

        def log_message(self, format: str, *args) -> None:
            print(format % args, flush=True)

    return Handler


def main() -> None:
    model_dir = Path(os.environ.get("GIGAAM_MODEL_DIR", "/opt/fsp/models/gigaam"))
    threads = int(os.environ.get("GIGAAM_CPU_THREADS", "4"))
    if threads < 1 or threads > 32:
        raise SystemExit("GIGAAM_CPU_THREADS must be between 1 and 32")
    runtime = GigaAmRuntime(model_dir, threads)
    host = os.environ.get("GIGAAM_HOST", "127.0.0.1")
    port = int(os.environ.get("GIGAAM_PORT", "4060"))
    ThreadingHTTPServer((host, port), handler(runtime)).serve_forever()


if __name__ == "__main__":
    main()
