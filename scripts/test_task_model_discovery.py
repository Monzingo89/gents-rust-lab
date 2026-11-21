import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import task_model_discovery as discovery


class DiscoveryTests(unittest.TestCase):
    def test_credentials_and_aliases_never_establish_extra_downloads_or_locality(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "models").mkdir()
            (root / "config").mkdir()
            (root / "config/enterprise-model-roster.json").write_text('{"models":[]}')
            (root / "config/providers.json").write_text(
                json.dumps(
                    {
                        "worker": {
                            "provider": "azure",
                            "model": "configured-only",
                            "url": "https://example.invalid/chat?api-key=secret-value",
                            "key_env": "PRIVATE_KEY_NAME",
                        }
                    }
                )
            )
            manifests = root / ".ollama/models/manifests/registry/library"
            blobs = root / ".ollama/models/blobs"
            blobs.mkdir(parents=True)
            (blobs / "sha256-one").write_bytes(b"fixture")
            for alias in ["base", "specialist"]:
                (manifests / alias).mkdir(parents=True)
                (manifests / alias / "latest").write_text(
                    json.dumps(
                        {
                            "layers": [
                                {
                                    "mediaType": "application/vnd.ollama.image.model",
                                    "digest": "sha256:one",
                                    "size": 7,
                                }
                            ]
                        }
                    )
                )
            with (
                patch.object(Path, "home", return_value=root),
                patch.object(discovery.shutil, "which", return_value=None),
            ):
                report = discovery.discover(root=root)
            self.assertEqual(len(report["ollama_manifests"]), 2)
            digests = {
                layer["digest"]
                for m in report["ollama_manifests"]
                for layer in m["layers"]
            }
            self.assertEqual(len(digests), 1)
            self.assertEqual(report["downloaded_artifacts"], [])
            self.assertFalse(report["weights_loaded"])
            self.assertNotIn("secret-value", json.dumps(report))
            self.assertNotIn("PRIVATE_KEY_NAME", json.dumps(report))

    def test_endpoint_embedded_credentials_are_rejected(self):
        self.assertIsNone(
            discovery.safe_endpoint("https://user:secret@example.invalid/")
        )
        self.assertEqual(
            discovery.safe_endpoint("http://127.0.0.1:11434?key=secret"),
            "http://127.0.0.1:11434",
        )


if __name__ == "__main__":
    unittest.main()
