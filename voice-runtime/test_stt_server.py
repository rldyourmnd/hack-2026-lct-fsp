import importlib.util
import socket
import struct
import subprocess
import threading
import unittest
import urllib.request
import wave
from http.server import ThreadingHTTPServer
from io import BytesIO
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


STT = load("gigaam_server", "stt_server.py")
TTS = load("silero_server", "server.py")


class RecordingTests(unittest.TestCase):
    def test_duration_comes_from_audio_samples_and_normalized_wav(self):
        pcm = b"\0\0" * 24_000
        with patch.object(STT.subprocess, "run", return_value=SimpleNamespace(stdout=pcm)):
            body, duration = STT.decode_recording("streaming.webm")
        self.assertEqual(duration, 1.5)
        with wave.open(BytesIO(body), "rb") as recording:
            self.assertEqual(recording.getnchannels(), 1)
            self.assertEqual(recording.getsampwidth(), 2)
            self.assertEqual(recording.getframerate(), 16_000)
            self.assertEqual(recording.readframes(recording.getnframes()), pcm)

    def test_rejects_empty_incomplete_and_oversized_decoded_audio(self):
        for pcm in (b"", b"\0", b"\0\0" * (25 * 16_000 + 1)):
            with self.subTest(size=len(pcm)):
                with patch.object(STT.subprocess, "run", return_value=SimpleNamespace(stdout=pcm)):
                    with self.assertRaisesRegex(ValueError, "between 0 and 25"):
                        STT.decode_recording("recording.webm")

    def test_decoder_failure_is_not_accepted_as_a_recording(self):
        with patch.object(STT.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "ffmpeg")):
            with self.assertRaises(subprocess.SubprocessError):
                STT.decode_recording("corrupt.webm")

    def test_model_receives_normalized_wav_and_temporary_audio_is_removed(self):
        observed = []

        class Model:
            def transcribe(self, filename):
                observed.append(Path(filename))
                with wave.open(filename, "rb") as recording:
                    self_test.assertEqual(recording.getframerate(), 16_000)
                    self_test.assertEqual(recording.getnframes(), 16_000)
                return SimpleNamespace(text="  Проверка речи  ")

        self_test = self
        runtime = STT.GigaAmRuntime.__new__(STT.GigaAmRuntime)
        runtime._model = Model()
        runtime._lock = threading.Lock()
        with patch.object(STT.subprocess, "run", return_value=SimpleNamespace(stdout=b"\0\0" * 16_000)):
            self.assertEqual(runtime.transcribe(b"streaming input", ".webm"), ("Проверка речи", 1.0))
        self.assertEqual(len(observed), 1)
        self.assertFalse(observed[0].exists())


class DisconnectTests(unittest.TestCase):
    def test_cancelled_speech_requests_do_not_escape_the_handler(self):
        for module, route, content_type, body in (
            (TTS, "/synthesize", "application/json", b'{"text":"control"}'),
            (STT, "/transcribe", "audio/webm", b"control"),
        ):
            with self.subTest(route=route):
                entered = threading.Event()
                release = threading.Event()
                completed = threading.Event()
                errors = []

                class Runtime:
                    def synthesize(self, *_args):
                        entered.set()
                        if not release.wait(3):
                            raise RuntimeError("test did not release inference")
                        return b"audio"

                    def transcribe(self, *_args):
                        self.synthesize()
                        return "control", 1.0

                class Server(ThreadingHTTPServer):
                    def handle_error(self, *_args):
                        errors.append("uncaught handler error")

                    def process_request_thread(self, request, client_address):
                        try:
                            super().process_request_thread(request, client_address)
                        finally:
                            completed.set()

                request_handler = module.handler(Runtime())
                request_handler.log_message = lambda *_args: None
                with Server(("127.0.0.1", 0), request_handler) as server:
                    thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.01))
                    thread.start()
                    try:
                        with socket.create_connection(server.server_address, timeout=3) as client:
                            headers = (
                                f"POST {route} HTTP/1.1\r\nHost: localhost\r\n"
                                f"Content-Type: {content_type}\r\nContent-Length: {len(body)}\r\n\r\n"
                            ).encode()
                            client.sendall(headers + body)
                            self.assertTrue(entered.wait(3))
                            client.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
                        release.set()
                        self.assertTrue(completed.wait(3))
                        with urllib.request.urlopen(f"http://127.0.0.1:{server.server_port}/health", timeout=3) as response:
                            self.assertEqual(response.status, 200)
                        self.assertEqual(errors, [])
                    finally:
                        release.set()
                        server.shutdown()
                        thread.join(timeout=3)


if __name__ == "__main__":
    unittest.main()
