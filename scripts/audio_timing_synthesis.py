"""Task replay using unchanged base and saved duration candidate, in fresh models."""

import hashlib
import json
import wave

import numpy as np
import torch
from piper.phoneme_ids import phonemes_to_ids
from piper.phonemize_espeak import EspeakPhonemizer
from piper.train.vits.models import SynthesizerTrn
from safetensors.torch import load_file


def synthesize(source, dataset, output):
    torch.set_num_threads(2)
    trial = source / "conversion-attempt/timing-experiment-v1"
    report = json.loads((trial / "report.json").read_text())
    architecture = json.loads(
        (source / "conversion-attempt/reconstruction/report.json").read_text()
    )["architecture"]
    base = source / "conversion-attempt/converted/state.safetensors"
    candidate = trial / "duration-predictor.safetensors"

    def sha(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()

    if (
        sha(base) != report["protocol"]["base_safetensors_sha256"]
        or sha(candidate) != report["candidate_sha256"]
    ):
        raise ValueError("weight pin mismatch")
    if sha(dataset / "manifest.json") != report["manifest_sha256"]:
        raise ValueError("dataset changed")
    voice = json.loads((source / "config.json").read_text())
    records = json.loads((dataset / "manifest.json").read_text())["records"]
    output.mkdir(exist_ok=False)
    receipts = []
    phonemizer = EspeakPhonemizer()
    for variant in ("baseline", "candidate"):
        model = SynthesizerTrn(**architecture).eval()
        state = load_file(str(base))
        model.load_state_dict(
            {
                k.removeprefix("model_g."): v
                for k, v in state.items()
                if k.startswith("model_g.")
            },
            strict=True,
        )
        del state
        if variant == "candidate":
            model.dp.load_state_dict(load_file(str(candidate)), strict=True)
        with torch.inference_mode():
            for record in records:
                if record["split"] != "test":
                    continue
                phones = phonemizer.phonemize(voice["espeak"]["voice"], record["text"])
                ids = phonemes_to_ids(
                    [p for row in phones for p in row], voice["phoneme_id_map"]
                )
                torch.manual_seed(17)
                signal = (
                    model.infer(
                        torch.tensor([ids]),
                        torch.tensor([len(ids)]),
                        noise_scale=0,
                        noise_scale_w=0,
                        max_len=10000,
                    )[0]
                    .flatten()
                    .numpy()
                )
                expected = next(c for c in report[variant] if c["id"] == record["id"])[
                    "predicted_seconds"
                ]
                if (
                    not np.isfinite(signal).all()
                    or abs(len(signal) / 22050 - expected) > 1 / 22050
                ):
                    raise ValueError("synthesized audio differs from timing evaluation")
                path = output / (variant + "-" + record["id"] + ".wav")
                with wave.open(str(path), "wb") as wav:
                    wav.setnchannels(1)
                    wav.setsampwidth(2)
                    wav.setframerate(22050)
                    wav.writeframes(
                        (np.clip(signal, -1, 1) * 32767).astype("<i2").tobytes()
                    )
                receipts.append(
                    {
                        "variant": variant,
                        "id": record["id"],
                        "text": record["text"],
                        "file": path.name,
                        "sha256": sha(path),
                        "samples": len(signal),
                        "peak": float(np.abs(signal).max()),
                        "clipped_samples": int((np.abs(signal) > 1).sum()),
                    }
                )
        del model
    result = {
        "cases": receipts,
        "scope": "Native held-out text synthesis; verifies finite audio and predicted durations, not intelligibility or naturalness.",
        "provider_cost_usd": 0,
        "promoted": False,
        "candidate_sha256": report["candidate_sha256"],
    }
    (output / "report.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"synthesized": len(receipts), "promoted": False}), flush=True)
