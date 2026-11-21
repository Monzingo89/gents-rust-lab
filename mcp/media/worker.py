"""Owned, isolated local media adapter. Binary stdout only; no network/downloads.

Invoked only by the native host with its own immutable request/manifest. Never a
public endpoint; no model-provided executables, modules, paths or credentials.
"""

import contextlib
import hashlib
import io
import json
import sys
import wave
from pathlib import Path


def verified_model(profile):
    root = Path(profile["root"])
    for row in profile["files"]:
        relative = Path(row["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Invalid model path")
        path = root
        for part in relative.parts:
            path /= part
            if path.is_symlink():
                raise ValueError("Model symlinks forbidden")
        if not path.is_file() or path.stat().st_size != row["bytes"]:
            raise ValueError("Model file missing or changed")
        with path.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != row["sha256"]:
                raise ValueError("Model digest mismatch")
    return root


def generate(request, profile):
    root = verified_model(profile)
    if request["kind"] == "image":
        import mlx.core as mx

        # MLX documents this as a working-set guideline, not a hard GPU quota.
        mx.set_memory_limit(10 * 1024**3)
        mx.set_cache_limit(512 * 1024**2)
        from mflux.models.flux2.variants import Flux2Klein

        model = Flux2Klein(model_path=str(root), quantize=4)
        generated = model.generate_image(
            prompt=request["prompt"],
            seed=request["seed"],
            width=request["width"],
            height=request["height"],
            num_inference_steps=4,
        )
        # Re-encode pixels without prompt/EXIF metadata in the public artifact.
        image = generated.image.convert("RGB")
        output = io.BytesIO()
        image.save(output, format="PNG")
        return output.getvalue()
    if request["kind"] == "audio":
        from piper import PiperVoice

        model = root / "en/en_US/ljspeech/high/en_US-ljspeech-high.onnx"
        voice = PiperVoice.load(str(model), use_cuda=False)
        output = io.BytesIO()
        with wave.open(output, "wb") as wav:
            voice.synthesize_wav(request["text"], wav)
        return output.getvalue()
    raise ValueError("Unsupported media kind")


def main():
    packet = json.loads(sys.stdin.buffer.read(16385))
    with contextlib.redirect_stdout(sys.stderr):
        data = generate(packet["request"], packet["profile"])
    if not 0 < len(data) <= 8 * 1024 * 1024:
        raise ValueError("Media output exceeded budget")
    sys.stdout.buffer.write(data)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Host retains bounded diagnostics privately, never inside MCP receipts.
        import traceback

        traceback.print_exc(file=sys.stderr)
        print(
            "Local media generation failed; inspect the configured engine and model manifest.",
            file=sys.stderr,
        )
        sys.exit(1)
