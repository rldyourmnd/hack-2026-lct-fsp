#!/usr/bin/env python3

import array
import hashlib
import io
import json
import os
import sys
import tempfile
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


VOICES = frozenset(("aidar", "baya", "kseniya", "xenia", "eugene"))
SAMPLE_RATES = frozenset((8_000, 24_000, 48_000))
MAX_BODY_BYTES = 16 * 1024
MAX_TEXT_CHARACTERS = 4_000
DEFAULT_CACHE_MAX_BYTES = 256 * 1024 * 1024


def parse_request(body: bytes) -> tuple[str, str, int]:
    if not body or len(body) > MAX_BODY_BYTES:
        raise ValueError("invalid request size")
    try:
        value = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("invalid JSON") from error
    if not isinstance(value, dict):
        raise ValueError("request must be an object")
    text = value.get("text")
    voice = value.get("voice", "eugene")
    sample_rate = value.get("sample_rate", 24_000)
    if not isinstance(text, str) or not text.strip() or len(text) > MAX_TEXT_CHARACTERS:
        raise ValueError("invalid text")
    if voice not in VOICES:
        raise ValueError("invalid voice")
    if sample_rate not in SAMPLE_RATES:
        raise ValueError("invalid sample rate")
    return text.strip(), voice, sample_rate


def encode_wav(samples, sample_rate: int) -> bytes:
    pcm = array.array(
        "h",
        (round(max(-1.0, min(1.0, float(sample))) * 32767) for sample in samples),
    )
    if sys.byteorder != "little":
        pcm.byteswap()
    output = io.BytesIO()
    with wave.open(output, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)
        wav.writeframes(pcm.tobytes())
    return output.getvalue()


def cache_key(model_version: str, text: str, voice: str, sample_rate: int) -> str:
    value = "\0".join((model_version, voice, str(sample_rate), text))
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def is_wav(body: bytes) -> bool:
    return len(body) >= 44 and body[:4] == b"RIFF" and body[8:12] == b"WAVE"


class SpeechCache:
    def __init__(self, directory: Path, max_bytes: int) -> None:
        self._directory = directory
        self._max_bytes = max_bytes
        directory.mkdir(parents=True, exist_ok=True)

    def get(self, key: str) -> bytes | None:
        path = self._directory / f"{key}.wav"
        try:
            body = path.read_bytes()
        except FileNotFoundError:
            return None
        if not is_wav(body):
            path.unlink(missing_ok=True)
            return None
        os.utime(path)
        return body

    def put(self, key: str, body: bytes) -> None:
        if not is_wav(body):
            raise ValueError("cache accepts WAV audio only")
        destination = self._directory / f"{key}.wav"
        with tempfile.NamedTemporaryFile(dir=self._directory, suffix=".tmp", delete=False) as output:
            temporary = Path(output.name)
            output.write(body)
        try:
            os.replace(temporary, destination)
        finally:
            temporary.unlink(missing_ok=True)
        self._prune(destination)

    def _prune(self, keep: Path) -> None:
        files = [path for path in self._directory.glob("*.wav") if path.is_file()]
        total = sum(path.stat().st_size for path in files)
        for path in sorted(files, key=lambda item: item.stat().st_mtime_ns):
            if total <= self._max_bytes:
                break
            if path == keep:
                continue
            size = path.stat().st_size
            path.unlink(missing_ok=True)
            total -= size


class SileroRuntime:
    def __init__(
        self,
        model_path: Path,
        model_version: str,
        threads: int,
        cache: SpeechCache,
    ) -> None:
        import torch

        torch.set_num_threads(threads)
        torch.set_num_interop_threads(1)
        self._model = torch.package.PackageImporter(str(model_path)).load_pickle(
            "tts_models", "model"
        )
        self._model_version = model_version
        self._cache = cache
        self._lock = threading.Lock()
        self._model.apply_tts(
            text="Проверка готовности интервьюера.",
            speaker="eugene",
            sample_rate=24_000,
            put_accent=True,
            put_yo=True,
        )

    def synthesize(self, text: str, voice: str, sample_rate: int) -> bytes:
        key = cache_key(self._model_version, text, voice, sample_rate)
        cached = self._cache.get(key)
        if cached is not None:
            return cached
        with self._lock:
            cached = self._cache.get(key)
            if cached is not None:
                return cached
            samples = self._model.apply_tts(
                text=text,
                speaker=voice,
                sample_rate=sample_rate,
                put_accent=True,
                put_yo=True,
            )
            audio = encode_wav(samples, sample_rate)
            self._cache.put(key, audio)
            return audio


def handler(runtime: SileroRuntime):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            if self.path != "/health":
                self.send_error(404)
                return
            self._respond(200, b'{"status":"ok","model":"v5_5_ru","device":"cpu"}', "application/json")

        def do_POST(self) -> None:
            if self.path != "/synthesize":
                self.send_error(404)
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if length < 1 or length > MAX_BODY_BYTES:
                    raise ValueError("invalid request size")
                text, voice, sample_rate = parse_request(self.rfile.read(length))
                audio = runtime.synthesize(text, voice, sample_rate)
            except ValueError as error:
                self._respond(422, json.dumps({"error": str(error)}).encode(), "application/json")
                return
            self._respond(200, audio, "audio/wav")

        def _respond(self, status: int, body: bytes, content_type: str) -> None:
            try:
                self.send_response(status)
                self.send_header("Content-Type", content_type)
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
    model_path = Path(os.environ.get("SILERO_MODEL_PATH", "/opt/fsp/models/v5_5_ru.pt"))
    model_version = os.environ.get("SILERO_MODEL_VERSION", "v5_5_ru")
    threads = int(os.environ.get("SILERO_CPU_THREADS", "4"))
    if threads < 1 or threads > 32:
        raise SystemExit("SILERO_CPU_THREADS must be between 1 and 32")
    cache_dir = Path(os.environ.get("SILERO_CACHE_DIR", "/var/cache/fsp-silero-tts"))
    cache_max_bytes = int(os.environ.get("SILERO_CACHE_MAX_BYTES", str(DEFAULT_CACHE_MAX_BYTES)))
    if cache_max_bytes < 1:
        raise SystemExit("SILERO_CACHE_MAX_BYTES must be positive")
    runtime = SileroRuntime(
        model_path,
        model_version,
        threads,
        SpeechCache(cache_dir, cache_max_bytes),
    )
    host = os.environ.get("SILERO_HOST", "127.0.0.1")
    port = int(os.environ.get("SILERO_PORT", "4050"))
    ThreadingHTTPServer((host, port), handler(runtime)).serve_forever()


if __name__ == "__main__":
    main()
