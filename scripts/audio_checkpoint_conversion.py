"""Restricted tensor-only extraction. Never fall back to unrestricted pickle."""

import hashlib
import json
from pathlib import PosixPath


def load_restricted(checkpoint, expected_sha256):
    import torch  # type: ignore[import-not-found]

    if checkpoint.is_symlink() or not checkpoint.is_file():
        raise ValueError("ordinary checkpoint required")
    with checkpoint.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != expected_sha256:
        raise ValueError("checkpoint hash mismatch")
    globals_found = torch.serialization.get_unsafe_globals_in_checkpoint(checkpoint)
    if set(globals_found) - {"pathlib.PosixPath"}:
        raise ValueError("checkpoint requests unapproved pickle globals")
    # PosixPath constructs path data only. Never add checkpoint-specified classes.
    with torch.serialization.safe_globals([PosixPath]):
        loaded = torch.load(checkpoint, map_location="cpu", weights_only=True)
    return loaded, globals_found


def convert(checkpoint, output, expected_sha256):
    import torch  # type: ignore[import-not-found]
    from safetensors.torch import save_file  # type: ignore[import-not-found]

    loaded, globals_found = load_restricted(checkpoint, expected_sha256)
    actual = expected_sha256
    if not isinstance(loaded, dict) or not isinstance(loaded.get("state_dict"), dict):
        raise TypeError("checkpoint lacks a tensor state_dict")
    tensors = {}
    inventory = []
    for name, value in loaded["state_dict"].items():
        if (
            not isinstance(name, str)
            or not isinstance(value, torch.Tensor)
            or value.layout != torch.strided
        ):
            raise ValueError("unsupported state entry")
        if value.is_floating_point() and not torch.isfinite(value).all():
            raise ValueError("nonfinite tensor")
        tensors[name] = value.detach().cpu().contiguous().clone()
        inventory.append(
            {
                "name": name,
                "shape": list(value.shape),
                "dtype": str(value.dtype),
                "elements": value.numel(),
            }
        )
    if not tensors:
        raise ValueError("empty state")
    output.mkdir(exist_ok=False)
    destination = output / "state.safetensors"
    save_file(
        tensors,
        str(destination),
        metadata={"source_sha256": actual, "format": "piper-state-dict-only"},
    )
    with destination.open("rb") as stream:
        converted = hashlib.file_digest(stream, "sha256").hexdigest()
    report = {
        "source_sha256": actual,
        "safetensors_sha256": converted,
        "tensor_count": len(tensors),
        "tensor_inventory": inventory,
        "allowed_nondefault_globals": globals_found,
        "weights_only": True,
        "optimizer_state_retained": False,
        "admitted": False,
        "scope": "Serialization conversion only; no model parity, licensing or quality acceptance",
    }
    (output / "conversion.json").write_text(json.dumps(report, indent=2) + "\n")
    return report
