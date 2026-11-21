import struct
import tempfile
import unittest
from pathlib import Path

import model_inventory as inventory


class InventoryTests(unittest.TestCase):
    def test_candidate_manifest_is_pinned_without_claiming_enterprise_admission(self):
        import json

        data = json.loads(
            (inventory.ROOT / "config/enterprise-model-candidates.json").read_text()
        )
        self.assertTrue(data["admission"].startswith("pending_"))
        candidates = data["new_candidates"]
        self.assertEqual(len({item["base_id"] for item in candidates}), 3)
        paths = [item["path"] for item in candidates] + data[
            "preexisting_gguf_candidates_needing_provenance_review"
        ]
        self.assertEqual(len(set(paths)), 10)
        for item in candidates:
            self.assertRegex(item["source_revision"], r"^[a-f0-9]{40}$")
            self.assertRegex(item["sha256"], r"^[a-f0-9]{64}$")
            self.assertLessEqual(
                item["evaluation_context_tokens"], item["documented_context_tokens"]
            )

    def test_metadata_is_read_without_loading_tensor_payload(self):
        text = lambda value: struct.pack("<Q", len(value)) + value
        fixture = b"GGUF" + struct.pack("<IQQ", 3, 42, 2)
        fixture += text(b"general.name") + struct.pack("<I", 8) + text(b"Fixture base")
        fixture += (
            text(b"general.architecture") + struct.pack("<I", 8) + text(b"fixture")
        )
        with tempfile.TemporaryFile() as stream:
            stream.write(fixture)
            stream.seek(0)
            result = inventory.GgufMetadata(stream).extract()
        self.assertEqual(result["tensor_count"], 42)
        self.assertEqual(result["metadata"]["general.name"], "Fixture base")

    def test_invalid_and_oversized_headers_are_rejected(self):
        for data in (b"not a model", b"GGUF" + struct.pack("<IQQ", 3, 0, 100001)):
            with tempfile.TemporaryFile() as stream:
                stream.write(data)
                stream.seek(0)
                with self.assertRaises(ValueError):
                    inventory.GgufMetadata(stream).extract()

    def test_symlinks_are_not_inventory_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "weights.gguf").write_bytes(b"GGUF")
            (base / "alias.gguf").symlink_to(base / "weights.gguf")
            with self.assertRaises(ValueError):
                inventory.artifact(base / "alias.gguf")

    def test_converted_phi_weights_group_as_one_unadmitted_base(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("phi-1-safetensors", "phi-1-mlx-4bit"):
                folder = root / "models" / name
                folder.mkdir(parents=True)
                (folder / "model.safetensors").write_bytes(b"fixture-not-loaded")
                (folder / "config.json").write_text('{"model_type":"phi"}')
            result = inventory.inventory(root=root)
            self.assertEqual(result["candidate_group_count"], 1)
            self.assertEqual(
                len(result["candidate_identity_groups"][0]["artifacts"]), 2
            )
            self.assertEqual(result["provenance_admitted_base_count"], 0)

    def test_cache_metadata_cannot_echo_arbitrary_text_as_an_etag(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / "models" / "fixture"
            folder.mkdir(parents=True)
            weights = folder / "model.safetensors"
            weights.write_bytes(b"fixture")
            (folder / "config.json").write_text('{"model_type":"fixture"}')
            cache = folder / ".cache/huggingface/download"
            cache.mkdir(parents=True)
            (cache / "model.safetensors.metadata").write_text(
                "a" * 40 + "\nPRIVATE VALUE\n"
            )
            result = inventory.artifact(weights, root=root)
            self.assertIsNone(result["cached_etag_claim"])
            self.assertNotIn("PRIVATE VALUE", str(result))


if __name__ == "__main__":
    unittest.main()
