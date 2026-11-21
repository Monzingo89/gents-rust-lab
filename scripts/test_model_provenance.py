import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import model_provenance as provenance


class ProvenanceTests(unittest.TestCase):
    def test_offline_reassignment_rehashes_artifact_and_cards_without_role_qualification(
        self,
    ):
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(provenance, "cli") as network,
        ):
            root = Path(directory)
            (root / "models").mkdir()
            (root / "data").mkdir()
            weight = root / "models/model.gguf"
            weight.write_bytes(b"fixture not a model")
            base = root / "data/old-role"
            base.mkdir()
            model = {
                "id": "new-role",
                "publisher": "owner/model",
                "source_repo": "owner/model-GGUF",
                "source_revision": "a" * 40,
                "path": "models/model.gguf",
                "sha256": provenance.file_sha(weight),
                "license": "apache-2.0",
            }
            proof = {
                **model,
                "id": "old-role",
                "status": "PASS",
                "bytes": weight.stat().st_size,
                "publisher_revision": "b" * 40,
                "declared_license": "apache-2.0",
                "weight_identity_verified": True,
                "role_qualified": True,
                "production_admitted": True,
            }
            for prefix, revision, repo in (
                ("source", "a" * 40, model["source_repo"]),
                ("publisher", "b" * 40, model["publisher"]),
            ):
                docs = base / f"{prefix}-{revision}"
                docs.mkdir()
                card = docs / "README.md"
                card.write_text("fixture model card")
                proof[prefix + "_documents"] = [
                    {
                        "filename": "README.md",
                        "sha256": provenance.file_sha(card),
                        "source": f"https://huggingface.co/{repo}/blob/{revision}/README.md",
                    }
                ]
            previous = base / "result.json"
            provenance.private_json(previous, proof)
            with patch.object(provenance, "ROOT", root):
                accepted = provenance.verify_cached(model, [(previous, proof)])
                self.assertEqual(accepted["id"], "new-role")
                self.assertFalse(accepted["role_qualified"])
                self.assertFalse(accepted["production_admitted"])
                self.assertEqual(
                    accepted["cached_provenance_sha256"], provenance.file_sha(previous)
                )
                card.write_text("tampered card")
                with self.assertRaises(ValueError):
                    provenance.verify_cached(model, [(previous, proof)])
                with self.assertRaises(ValueError):
                    provenance.verify_cached(
                        {**model, "sha256": "0" * 64}, [(previous, proof)]
                    )
                weight.write_bytes(b"tampered weight")
                with self.assertRaises(ValueError):
                    provenance.verify_cached(model, [(previous, proof)])
            network.assert_not_called()

    def test_roster_requires_distinct_artifacts_and_pinned_sources(self):
        roster = json.loads(provenance.ROSTER.read_text())
        self.assertEqual(len(provenance.validate_roster(roster)), 10)
        for field, value in (
            ("sha256", roster["models"][0]["sha256"]),
            ("publisher", roster["models"][0]["publisher"]),
            ("source_revision", "main"),
            ("path", "../outside.gguf"),
        ):
            bad = copy.deepcopy(roster)
            bad["models"][1][field] = value
            with self.assertRaises(ValueError):
                provenance.validate_roster(bad)

    def test_paths_and_document_symlinks_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "models").mkdir()
            (root / "models/link").symlink_to(root)
            for relative in ("/outside", "models/../outside", "models/link/outside"):
                with self.assertRaises(ValueError):
                    provenance.local_path(root, relative, "models")
            (root / "data").mkdir()
            (root / "data/source").symlink_to(root)
            with patch.object(provenance, "ROOT", root), self.assertRaises(ValueError):
                provenance.documents("owner/model", "a" * 40, root / "data/source", [])

    def test_public_cli_never_inherits_cloud_secrets(self):
        from types import SimpleNamespace

        with (
            patch.dict(
                provenance.os.environ,
                {"RUST_OPENAI_API_KEY": "not-for-cli", "HF_TOKEN": "not-for-cli"},
            ),
            patch.object(
                provenance.subprocess,
                "run",
                return_value=SimpleNamespace(returncode=0, stdout="{}"),
            ) as run,
        ):
            provenance.cli("models", "info", "owner/model", "--format", "json")
            env = run.call_args.kwargs["env"]
            self.assertNotIn("RUST_OPENAI_API_KEY", env)
            self.assertNotIn("HF_TOKEN", env)
            self.assertEqual(env["HF_HUB_DISABLE_IMPLICIT_TOKEN"], "1")

    def test_private_report_creation_and_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            provenance.private_json(path, {"role_qualified": False})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertFalse(json.loads(path.read_text())["role_qualified"])
            path.unlink()
            path.symlink_to(Path(directory) / "outside")
            with self.assertRaises(ValueError):
                provenance.private_json(path, {})


if __name__ == "__main__":
    unittest.main()
