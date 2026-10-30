import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
from vision_tools import LlamaCppVisionBackend, observation


class VisionTools(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.work = self.root / "workspace"
        self.work.mkdir()
        (self.root / "models").mkdir()
        (self.root / "config").mkdir()
        for name in ["base.gguf", "projector.gguf"]:
            (self.root / "models" / name).write_bytes(b"mock weights; no inference in unit tests")
        (self.root / "config/vision.local.json").write_text(json.dumps({"model_id": "fixture-vision", "model": "base.gguf", "projector": "projector.gguf"}))
        self.image = self.work / "page.png"
        self.image.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\x0dIHDR" + struct.pack(">II", 1200, 800))
        self.binary = patch("vision_tools.shutil.which", return_value="/trusted/llama-mtmd-cli")
        self.binary.start()

    def tearDown(self):
        self.binary.stop()
        self.temp.cleanup()

    def prepare(self, **arguments):
        return LlamaCppVisionBackend().prepare(self.root, self.work, {"paths": ["page.png"], "question": "Review visible layout", **arguments})

    def test_inputs_are_hash_bound_and_model_files_are_explicit_reads(self):
        invocation = self.prepare()
        self.assertEqual(invocation.model_id, "fixture-vision")
        self.assertEqual(invocation.images[0]["width"], 1200)
        self.assertEqual(len(invocation.images[0]["sha256"]), 64)
        self.assertEqual(invocation.read_files, [self.root / "models/base.gguf", self.root / "models/projector.gguf"])
        self.assertEqual(invocation.argv[invocation.argv.index("--image")+1], str(self.image))
        self.assertNotIn("--log-disable", invocation.argv)
        result = observation(invocation, "<think>hidden scratchpad</think> Visible panels.", 25)
        self.assertEqual(result["observation"], "Visible panels.")
        self.assertIsNone(result["usage"]["output"])
        self.assertIn("not final task QA", result["authority"])
        with self.assertRaises(ValueError):
            observation(invocation, "<think>unfinished private scratchpad", 25)

    def test_traversal_symlinks_and_unbounded_requests_are_rejected(self):
        for arguments in [{"paths": ["../outside.png"]}, {"paths": [str(self.image)]},
                          {"paths": ["page.png", "page.png"]}, {"max_tokens": 1025}, {"question": ""}]:
            with self.assertRaises(ValueError):
                self.prepare(**arguments)
        (self.work / "alias.png").symlink_to(self.image)
        with self.assertRaises(ValueError):
            self.prepare(paths=["alias.png"])
        self.image.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\x0dIHDR" + struct.pack(">II", 100000, 100000))
        with self.assertRaises(ValueError):
            self.prepare()

    def test_missing_projector_never_attempts_network_or_implicit_download(self):
        (self.root / "models/projector.gguf").unlink()
        with self.assertRaises(ValueError):
            self.prepare()


if __name__ == "__main__":
    unittest.main()
