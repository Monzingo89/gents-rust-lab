import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest

import model_security_scan as scan
import model_security_inspection as inspection


class ModelSecurityTests(unittest.TestCase):
    def prepare(self, root):
        folder = root / "models/fixture"
        folder.mkdir(parents=True)
        path = folder / "weight.gguf"
        path.write_bytes(b"GGUF" + struct.pack("<IQQ", 3, 0, 0))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        (root / "config").mkdir()
        (root / "config/enterprise-model-roster.json").write_text(json.dumps({"models":[{"path":"models/fixture/weight.gguf","sha256":digest}]}))
        return folder, path, {"models/fixture/weight.gguf":digest}

    def test_pins_are_not_learned_from_changed_or_unknown_weights(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            folder, path, pins = self.prepare(root)
            self.assertEqual(inspection.inspect(root,pins)["status"],"CHECKS_PASSED")
            path.write_bytes(path.read_bytes()+b"changed")
            (folder / "unknown.gguf").write_bytes(path.read_bytes())
            result = inspection.inspect(root,pins)
            self.assertEqual(result["status"],"REVIEW_REQUIRED")
            self.assertEqual({f["kind"] for f in result["findings"]},{"reviewed_hash_mismatch","unreviewed_weight"})
            self.assertFalse(result["production_admitted"])
            self.assertIsNone(result["vulnerability_free"])

    def test_links_special_files_code_configs_and_bad_headers_are_flagged_without_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            folder, path, pins = self.prepare(root)
            (folder / "link.gguf").symlink_to(path)
            os.link(path,folder / "hardlink.gguf")
            (folder / "escape").symlink_to(root.parent,target_is_directory=True)
            (folder / "payload.py").write_text("raise RuntimeError('must never execute')")
            (folder / "config.json").write_text('{"auto_map":{"AutoModel":"payload.Run"}}')
            (folder / "bad.gguf").write_bytes(b"GGUF")
            result = inspection.inspect(root,pins)
            kinds = {f["kind"] for f in result["findings"]}
            self.assertTrue({"special_or_linked_file","symlink_directory","executable_or_unsafe_serialization_present","remote_code_configuration_requires_review","artifact_inspection_failed"} <= kinds)
            self.assertFalse(result["model_code_executed"])
            self.assertNotIn("must never execute",json.dumps(result))

    def test_safetensors_offsets_cannot_escape_or_overlap(self):
        def stream(offsets):
            header = json.dumps({str(i):{"dtype":"F32","shape":[1],"data_offsets":pair} for i,pair in enumerate(offsets)}).encode()
            return io.BytesIO(struct.pack("<Q",len(header))+header+b"12345678")
        for offsets in [[[-1,4]],[[0,100]],[[0,4],[2,6]]]:
            data = stream(offsets)
            with self.assertRaises(ValueError):
                inspection.validate_header(data,".safetensors",len(data.getvalue()))
        data = stream([[0,4],[4,8]])
        inspection.validate_header(data,".safetensors",len(data.getvalue()))

    def test_daily_report_is_private_durable_and_does_not_rescan_before_due(self):
        with tempfile.TemporaryDirectory() as temporary, contextlib.redirect_stdout(io.StringIO()):
            root = Path(temporary).resolve()
            self.prepare(root)
            result = scan.daily(root)
            self.assertEqual(result["status"],"CHECKS_PASSED")
            latest = root / "data/model-security/latest.json"
            self.assertEqual(latest.stat().st_mode & 0o777,0o600)
            saved = latest.read_bytes()
            self.assertEqual(scan.daily(root)["status"],"NOT_DUE")
            self.assertEqual(latest.read_bytes(),saved)
            self.assertTrue((latest.parent / (result["scan_id"]+".json")).is_file())

    def test_symlink_model_root_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "models").symlink_to(root.parent,target_is_directory=True)
            with self.assertRaises(OSError):
                inspection.inspect(root,{})

    def test_daily_lock_does_not_start_a_competing_scan(self):
        import fcntl
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            self.prepare(root)
            folder = root / "data/model-security"
            folder.mkdir(parents=True)
            with (folder / "scan.lock").open("w") as lock:
                fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
                self.assertEqual(scan.daily(root)["status"],"SCAN_ALREADY_RUNNING")
                self.assertFalse((folder / "latest.json").exists())

    def test_output_symlink_is_rejected_without_overwriting_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            self.prepare(root)
            (root / "data").mkdir()
            outside = root / "outside"
            outside.mkdir()
            marker = outside / "latest.json"
            marker.write_text("unchanged")
            (root / "data/model-security").symlink_to(outside,target_is_directory=True)
            with self.assertRaises(OSError):
                scan.daily(root)
            self.assertEqual(marker.read_text(),"unchanged")


if __name__ == "__main__":
    unittest.main()
