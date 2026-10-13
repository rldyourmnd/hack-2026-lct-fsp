import importlib.util
import struct
import tempfile
import unittest
import wave
from io import BytesIO
from pathlib import Path


SPEC = importlib.util.spec_from_file_location("silero_server", Path(__file__).with_name("server.py"))
SERVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SERVER)


class ServerTests(unittest.TestCase):
    def test_request_boundaries(self):
        self.assertEqual(
            SERVER.parse_request(
                '{"text":"  Привет  ","voice":"eugene","sample_rate":24000}'.encode()
            ),
            ("Привет", "eugene", 24000),
        )
        for body in (b"", b"[]", b'{"text":""}', b'{"text":"ok","voice":"unknown"}'):
            with self.assertRaises(ValueError):
                SERVER.parse_request(body)

    def test_wav_is_mono_pcm16(self):
        body = SERVER.encode_wav((-2.0, -0.5, 0.5, 2.0), 24_000)
        with wave.open(BytesIO(body), "rb") as wav:
            self.assertEqual((wav.getnchannels(), wav.getsampwidth(), wav.getframerate()), (1, 2, 24_000))
            self.assertEqual(struct.unpack("<hhhh", wav.readframes(4)), (-32767, -16384, 16384, 32767))

    def test_cache_persists_and_separates_synthesis_inputs(self):
        body = SERVER.encode_wav((0.0, 0.25), 24_000)
        with tempfile.TemporaryDirectory() as directory:
            cache = SERVER.SpeechCache(Path(directory), 1_000_000)
            key = SERVER.cache_key("v5_5_ru", "Привет", "eugene", 24_000)
            cache.put(key, body)

            restarted_cache = SERVER.SpeechCache(Path(directory), 1_000_000)
            self.assertEqual(restarted_cache.get(key), body)
            self.assertIsNone(
                restarted_cache.get(
                    SERVER.cache_key("v5_5_ru", "Привет", "aidar", 24_000)
                )
            )

    def test_cache_rejects_corrupt_audio_and_evicts_oldest_entry(self):
        first = SERVER.encode_wav((0.0,) * 100, 24_000)
        second = SERVER.encode_wav((0.1,) * 100, 24_000)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache = SERVER.SpeechCache(root, len(second) + 20)
            corrupt_key = "0" * 64
            (root / f"{corrupt_key}.wav").write_bytes(b"not a wav")
            self.assertIsNone(cache.get(corrupt_key))
            self.assertFalse((root / f"{corrupt_key}.wav").exists())

            cache.put("1" * 64, first)
            cache.put("2" * 64, second)
            self.assertIsNone(cache.get("1" * 64))
            self.assertEqual(cache.get("2" * 64), second)

    def test_runtime_skips_model_inference_on_cache_hit(self):
        class Model:
            def __init__(self):
                self.calls = 0

            def apply_tts(self, **_arguments):
                self.calls += 1
                return (0.0, 0.25)

        with tempfile.TemporaryDirectory() as directory:
            runtime = SERVER.SileroRuntime.__new__(SERVER.SileroRuntime)
            runtime._model = Model()
            runtime._model_version = "v5_5_ru"
            runtime._cache = SERVER.SpeechCache(Path(directory), 1_000_000)
            runtime._lock = SERVER.threading.Lock()

            first = runtime.synthesize("Один вопрос", "eugene", 24_000)
            second = runtime.synthesize("Один вопрос", "eugene", 24_000)
            self.assertEqual(first, second)
            self.assertEqual(runtime._model.calls, 1)


if __name__ == "__main__":
    unittest.main()
