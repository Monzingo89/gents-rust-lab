#!/usr/bin/env python3
"""Read-only modality inventory; never loads models, trusts code, or grants admission."""

import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MEDIA = {
    "flux2-klein-4b-4bit": (
        "image-generation",
        "experimental adapter training exists; no qualified visual improvement or production adapter admission",
    ),
    "piper-ljspeech": (
        "speech-generation",
        "experimental duration training exists; intelligibility, naturalness and production candidate export remain unqualified",
    ),
    "qwen3-vl-8b": (
        "vision-language",
        "text and visual tasks need separate measurements; GGUF is not training input",
    ),
}


def inventory(root=ROOT):
    models = root / "models"
    if models.is_symlink():
        raise ValueError("model root symlink rejected")
    roster = json.loads((root / "config/enterprise-model-roster.json").read_text())[
        "models"
    ]
    roles = {Path(r["path"]).parts[1]: r["proposed_role"] for r in roster}
    result = []
    for folder in sorted(models.iterdir()):
        if folder.is_symlink() or not folder.is_dir():
            continue
        artifacts, rejected = [], []
        for base, dirs, files in os.walk(folder, followlinks=False):
            for name in list(dirs):
                p = Path(base) / name
                if p.is_symlink():
                    rejected.append(str(p.relative_to(root)))
                    dirs.remove(name)
                elif name == ".cache":
                    dirs.remove(name)
            for name in sorted(files):
                p = Path(base) / name
                if p.suffix not in (".gguf", ".safetensors", ".onnx"):
                    continue
                if p.is_symlink() or not p.is_file():
                    rejected.append(str(p.relative_to(root)))
                    continue
                artifacts.append(
                    {
                        "path": str(p.relative_to(root)),
                        "bytes": p.stat().st_size,
                        "format": p.suffix[1:],
                        "sha256": None,
                    }
                )
        if not artifacts and not rejected:
            continue
        modality, gap = MEDIA.get(
            folder.name, ("language", "no measured role improvement yet")
        )
        formats = {a["format"] for a in artifacts}
        if modality == "language" and "gguf" in formats:
            gap += "; compatible trainable source or adapter backend required"
        result.append(
            {
                "directory": folder.name,
                "modality": modality,
                "role_claim": roles.get(
                    folder.name, "unassigned experimental local artifact"
                ),
                "artifacts": artifacts,
                "rejected_links": rejected,
                "capability_gap": gap,
                "admitted": False,
            }
        )
    return {
        "schema_version": 1,
        "offline": True,
        "models": result,
        "scope": "Physical artifacts and configured role claims, not distinct base identities or capability admission",
        "hashes": "Not computed; use model_security_scan.py for content inspection and reviewed pins",
        "video_generation": "No installed video-generation backend identified by this registry",
        "weights_modified": False,
        "provider_cost_usd": 0,
    }


if __name__ == "__main__":
    print(json.dumps(inventory(), indent=2))
