"""Read-only model artifact inspection; no inference, network or model execution."""

import hashlib
import json
import os
import stat
import struct
from itertools import pairwise
from pathlib import Path

from model_inventory import GgufMetadata

WEIGHTS = {".gguf", ".safetensors"}
EXECUTABLE = {
    ".py",
    ".pyc",
    ".pkl",
    ".pickle",
    ".pt",
    ".pth",
    ".bin",
    ".so",
    ".dylib",
    ".dll",
    ".exe",
    ".sh",
    ".js",
}


def directory(path, create=False):
    """Open each absolute directory component without following symlinks."""
    if os.name != "posix" or not hasattr(os, "O_NOFOLLOW"):
        raise RuntimeError("secure model-scan filesystem adapter requires POSIX")
    path = Path(path).absolute()
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in path.parts[1:]:
            if part in {".", ".."}:
                raise ValueError("invalid directory component")
            if create:
                try:
                    os.mkdir(part, 0o700, dir_fd=fd)
                except FileExistsError:
                    pass
            following = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd
            )
            os.close(fd)
            fd = following
        return fd
    except BaseException:
        os.close(fd)
        raise


def ordinary_file(fd, name):
    opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
    info = os.fstat(opened)
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        os.close(opened)
        raise ValueError("model files must be regular and singly linked")
    return os.fdopen(opened, "rb")


def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def validate_header(stream, suffix, size):
    if suffix == ".gguf":
        GgufMetadata(stream).extract()
        return
    prefix = stream.read(8)
    if len(prefix) != 8:
        raise ValueError("truncated Safetensors header")
    length = struct.unpack("<Q", prefix)[0]
    if not 2 <= length <= min(16 * 1024 * 1024, size - 8):
        raise ValueError("Safetensors header exceeds bounds")
    header = json.loads(stream.read(length))
    if not isinstance(header, dict) or len(header) > 100000:
        raise ValueError("invalid Safetensors header object")
    ranges = []
    for name, value in header.items():
        if name == "__metadata__":
            continue
        if not isinstance(value, dict) or not isinstance(value.get("dtype"), str):
            raise ValueError("invalid tensor descriptor")
        shape, offsets = value.get("shape"), value.get("data_offsets")
        if (
            not isinstance(shape, list)
            or len(shape) > 16
            or any(type(n) is not int or n < 0 for n in shape)
        ):
            raise ValueError("invalid tensor shape")
        if (
            not isinstance(offsets, list)
            or len(offsets) != 2
            or any(type(n) is not int for n in offsets)
        ):
            raise ValueError("invalid tensor offsets")
        start, end = offsets
        if not 0 <= start <= end <= size - 8 - length:
            raise ValueError("tensor range outside artifact")
        if start != end:
            ranges.append((start, end))
    ordered = sorted(ranges)
    if any(left[1] > right[0] for left, right in pairwise(ordered)):
        raise ValueError("overlapping tensor ranges")


def inspect(root, pins, progress=lambda _: None):
    model_fd = directory(root / "models")
    records, findings, seen = [], [], set()
    total_files = 0
    try:
        for folder, dirs, names, fd in os.fwalk(
            ".", dir_fd=model_fd, follow_symlinks=False
        ):
            if len(Path(folder).parts) > 16:
                raise ValueError("model directory depth exceeds bound")
            for name in list(dirs):
                if stat.S_ISLNK(
                    os.stat(name, dir_fd=fd, follow_symlinks=False).st_mode
                ):
                    findings.append(
                        {
                            "path": str(Path("models") / folder / name),
                            "kind": "symlink_directory",
                        }
                    )
                    dirs.remove(name)
            for name in sorted(names):
                total_files += 1
                if total_files > 8192:
                    raise ValueError("model inventory exceeds file bound")
                path = str(Path("models") / folder / name)
                if len(path.encode()) > 512:
                    raise ValueError("model artifact path exceeds bound")
                suffix = Path(name).suffix.lower()
                info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                    findings.append({"path": path, "kind": "special_or_linked_file"})
                    continue
                if suffix in EXECUTABLE:
                    findings.append(
                        {
                            "path": path,
                            "kind": "executable_or_unsafe_serialization_present",
                        }
                    )
                if name == "config.json":
                    try:
                        with ordinary_file(fd, name) as stream:
                            raw = stream.read(1_000_001)
                        if len(raw) > 1_000_000:
                            raise ValueError("configuration exceeds bound")
                        config = json.loads(raw)
                        if not isinstance(config, dict):
                            raise ValueError("configuration must be object")
                        if config.get("auto_map") or config.get("trust_remote_code"):
                            findings.append(
                                {
                                    "path": path,
                                    "kind": "remote_code_configuration_requires_review",
                                }
                            )
                    except (ValueError, OSError):
                        findings.append({"path": path, "kind": "invalid_configuration"})
                if suffix not in WEIGHTS:
                    continue
                seen.add(path)
                progress(path)
                try:
                    with ordinary_file(fd, name) as stream:
                        before = os.fstat(stream.fileno())
                        validate_header(stream, suffix, before.st_size)
                        stream.seek(0)
                        digest = hashlib.file_digest(stream, "sha256").hexdigest()
                        if identity(before) != identity(
                            os.fstat(stream.fileno())
                        ) or identity(before) != identity(
                            os.stat(name, dir_fd=fd, follow_symlinks=False)
                        ):
                            raise ValueError("artifact changed while scanning")
                    matched = digest == pins.get(path)
                    records.append(
                        {
                            "path": path,
                            "bytes": before.st_size,
                            "sha256": digest,
                            "reviewed_pin_matches": matched,
                            "format_header_checked": True,
                        }
                    )
                    if not matched:
                        findings.append(
                            {
                                "path": path,
                                "kind": "reviewed_hash_mismatch"
                                if path in pins
                                else "unreviewed_weight",
                            }
                        )
                except (ValueError, OSError, UnicodeError, RecursionError):
                    # Never echo arbitrary model metadata or parser diagnostics.
                    findings.append(
                        {"path": path, "kind": "artifact_inspection_failed"}
                    )
        for missing in sorted(set(pins) - seen):
            findings.append({"path": missing, "kind": "reviewed_weight_missing"})
    finally:
        os.close(model_fd)
    return {
        "version": 1,
        "offline": True,
        "provider_cost_usd": 0,
        "model_code_executed": False,
        "status": "REVIEW_REQUIRED" if findings else "CHECKS_PASSED",
        "artifacts": records,
        "findings": findings,
        "files_inspected": total_files,
        "production_admitted": False,
        "vulnerability_free": None,
        "limits": "Integrity and bounded header/configuration checks only; no backdoor detection, current advisory lookup, license or compliance certification. No automatic model promotion or deletion.",
    }
