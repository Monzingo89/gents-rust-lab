#!/usr/bin/env python3
"""Offline model artifact inventory. Never loads weights or executes model code."""

import argparse
import hashlib
import json
import math
import os
import re
import struct
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCALARS = {
    0: "B",
    1: "b",
    2: "H",
    3: "h",
    4: "I",
    5: "i",
    6: "f",
    7: "?",
    10: "Q",
    11: "q",
    12: "d",
}


class GgufMetadata:
    def __init__(self, stream):
        self.stream = stream
        self.limit = min(os.fstat(stream.fileno()).st_size, 128 * 1024 * 1024)

    def read(self, count):
        if count < 0 or self.stream.tell() + count > self.limit:
            raise ValueError("GGUF metadata exceeds bounded file/header limits")
        data = self.stream.read(count)
        if len(data) != count:
            raise ValueError("truncated GGUF metadata")
        return data

    def number(self, code):
        return struct.unpack("<" + code, self.read(struct.calcsize(code)))[0]

    def string(self, retain=True):
        count = self.number("Q")
        if count > 16 * 1024 * 1024:
            raise ValueError("oversized GGUF string")
        value = self.read(count)
        return value.decode("utf-8", errors="strict") if retain else None

    def value(self, kind, retain=True, depth=0):
        if depth > 4:
            raise ValueError("GGUF array nesting exceeds bound")
        if kind in SCALARS:
            value = self.number(SCALARS[kind])
            if isinstance(value, float) and not math.isfinite(value):
                raise ValueError("non-finite GGUF metadata rejected")
            return value if retain else None
        if kind == 8:
            return self.string(retain)
        if kind == 9:
            inner, count = self.number("I"), self.number("Q")
            if count > 2_000_000:
                raise ValueError("GGUF array exceeds bound")
            for _ in range(count):
                self.value(inner, False, depth + 1)
            return {"items": count, "payload_retained": False} if retain else None
        raise ValueError("unsupported GGUF metadata type")

    def extract(self):
        if self.read(4) != b"GGUF" or self.number("I") not in (2, 3):
            raise ValueError("unsupported GGUF header")
        tensors, count = self.number("Q"), self.number("Q")
        if count > 100_000:
            raise ValueError("GGUF metadata count exceeds bound")
        fields = {}
        for _ in range(count):
            key = self.string()
            keep = key.startswith("general.") or key.endswith(".context_length")
            value = self.value(self.number("I"), keep)
            if keep:
                fields[key] = value
        return {"tensor_count": tensors, "metadata": fields}


def file_sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def artifact(path, fingerprint=False, root=None):
    root = ROOT if root is None else root
    if path.is_symlink() or not path.is_file():
        raise ValueError("model artifacts must be ordinary local files")
    record = {
        "path": str(path.relative_to(root)),
        "bytes": path.stat().st_size,
        "sha256": file_sha(path) if fingerprint else None,
    }
    if path.suffix == ".gguf":
        with path.open("rb") as stream:
            record.update(GgufMetadata(stream).extract())
        fields = record["metadata"]
        record["name_claim"] = fields.get("general.name")
        record["architecture"] = fields.get("general.architecture")
        record["base_name_claim"] = (
            fields.get("general.basename") or record["name_claim"]
        )
        record["size_claim"] = fields.get("general.size_label")
    else:
        config = path.parent / "config.json"
        if (
            config.is_symlink()
            or not config.is_file()
            or config.stat().st_size > 1_000_000
        ):
            raise ValueError(
                "safe local configuration required for Safetensors inventory"
            )
        value = json.loads(config.read_text())
        record.update(
            architecture=value.get("model_type"),
            name_claim=value.get("_name_or_path"),
            config_sha256=file_sha(config),
            dimensions={
                key: value.get(key)
                for key in (
                    "hidden_size",
                    "num_hidden_layers",
                    "num_attention_heads",
                    "vocab_size",
                )
            },
        )
        # A conversion is not another base model. This is only a local grouping
        # hint until publisher identity and immutable source are reviewed.
        record["base_name_claim"] = re.sub(
            r"-(mlx-\d+bit|safetensors)$", "", path.parent.name
        )
    metadata = path.parent / ".cache/huggingface/download" / (path.name + ".metadata")
    safe_metadata, cursor = True, root
    for part in metadata.relative_to(root).parts:
        cursor = cursor / part
        if cursor.is_symlink():
            safe_metadata = False
            break
    if safe_metadata and metadata.is_file() and metadata.stat().st_size < 10000:
        lines = metadata.read_text().splitlines()
        if len(lines) >= 2:
            record["cached_commit_claim"] = (
                lines[0] if re.fullmatch(r"[a-f0-9]{40}", lines[0]) else None
            )
            record["cached_etag_claim"] = (
                lines[1]
                if re.fullmatch(r"(?:[a-f0-9]{40}|[a-f0-9]{64})", lines[1])
                else None
            )
            record["cached_sha_matches"] = bool(
                record["sha256"] and record["sha256"] == lines[1]
            )
    record["admission"] = "unreviewed_identity_license_and_role_evaluation"
    return record


def inventory(fingerprint=False, root=None):
    root = ROOT if root is None else root
    models = root / "models"
    if models.is_symlink():
        raise ValueError("model root symlink rejected")
    records, failures = [], []
    for folder in sorted(models.iterdir()):
        if folder.is_symlink() or not folder.is_dir():
            continue
        for path in sorted(folder.iterdir()):
            if (
                path.suffix not in {".gguf", ".safetensors"}
                or "mmproj" in path.name
                or "adapter" in path.name
            ):
                continue
            try:
                records.append(artifact(path, fingerprint, root))
            except (ValueError, OSError, UnicodeError) as error:
                failures.append(
                    {"path": str(path.relative_to(root)), "error": str(error)[:300]}
                )
    grouped = {}
    for row in records:
        key = json.dumps(
            [row.get("architecture"), row.get("base_name_claim"), row.get("size_claim")]
        )
        group = grouped.setdefault(
            key,
            {
                "architecture": row.get("architecture"),
                "base_name_claim": row.get("base_name_claim"),
                "size_claim": row.get("size_claim"),
                "artifacts": [],
            },
        )
        group["artifacts"].append(row["path"])
    groups = [grouped[key] for key in sorted(grouped)]
    return {
        "version": 1,
        "offline": True,
        "fingerprints_computed": fingerprint,
        "artifacts": records,
        "failures": failures,
        "candidate_identity_groups": groups,
        "candidate_group_count": len(groups),
        "provenance_admitted_base_count": 0,
        "counting_rule": "Embedded names/cache claims are not publisher provenance or role admission; conversions and quantizations never establish additional distinct bases",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--hash",
        action="store_true",
        help="Stream actual weight hashes; may take several minutes",
    )
    args = parser.parse_args()
    report = inventory(args.hash)
    folder = ROOT / "data/model-registry"
    for target in (ROOT / "data", folder):
        if target.is_symlink():
            raise ValueError("inventory output symlink rejected")
    folder.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(folder, 0o700)
    with tempfile.NamedTemporaryFile(
        mode="w", dir=folder, prefix=".inventory-", delete=False
    ) as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")
        temporary = stream.name
    os.replace(temporary, folder / "local-inventory.json")
    print(
        json.dumps(
            {
                "output": str(folder / "local-inventory.json"),
                "artifacts": len(report["artifacts"]),
                "candidate_groups": report["candidate_group_count"],
                "admitted_bases": 0,
                "failures": report["failures"],
            }
        )
    )


if __name__ == "__main__":
    main()
