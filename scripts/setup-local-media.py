"""Explicit setup only: fetch pinned public model files, then verify every hash.

Run after installing mcp/media/requirements.lock in .venv-media. Never invoked by
an agent tool or an HTTP handler. Uses no HF token or paid service.
"""

import hashlib
import json
from pathlib import Path

from huggingface_hub import snapshot_download

ROOT = Path(__file__).resolve().parents[1]
manifest = json.loads((ROOT / "config/media.local.json").read_text())
for kind, profile in manifest["profiles"].items():
    destination = ROOT / profile["root"]
    snapshot_download(
        profile["repo"],
        revision=profile["revision"],
        local_dir=destination,
        allow_patterns=[row["path"] for row in profile["files"]],
        token=False,
        max_workers=2,
    )
    for row in profile["files"]:
        path = destination / row["path"]
        if (
            path.is_symlink()
            or not path.is_file()
            or path.stat().st_size != row["bytes"]
        ):
            raise RuntimeError("Media model file changed or missing")
        with path.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != row["sha256"]:
                raise RuntimeError(
                    "Media model hash mismatch; generation stays unavailable"
                )
    print(
        json.dumps(
            {
                "kind": kind,
                "revision": profile["revision"],
                "files_verified": len(profile["files"]),
            }
        )
    )
