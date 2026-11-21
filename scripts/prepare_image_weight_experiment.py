#!/usr/bin/env python3
"""Prepare owned geometric training data and frozen prospective image checks.

This creates calibration fixtures, not artwork or a media-quality claim. It never
loads a model, modifies weights, downloads assets, or starts training.
"""

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COLORS = {
    "red": (220, 30, 30),
    "blue": (30, 60, 220),
    "green": (30, 180, 60),
    "purple": (150, 40, 180),
}


def spec(color, side, seed):
    return {
        "prompt": f"A flat solid {color} square centered on a pure white background. Minimal geometric icon. No text, shadows, outlines, gradients or other objects.",
        "color": color,
        "side": side,
        "seed": seed,
        "width": 256,
        "height": 256,
    }


def prepare(destination):
    from PIL import Image, ImageDraw

    destination.mkdir(parents=True, exist_ok=False)
    train = destination / "train"
    train.mkdir()
    records = []
    for color in ["red", "blue", "green", "purple"]:
        for side in [96, 112, 128]:
            item = spec(color, side, 0)
            name = f"{color}-{side}"
            image = Image.new("RGB", (256, 256), "white")
            start = (256 - side) // 2
            ImageDraw.Draw(image).rectangle(
                (start, start, start + side - 1, start + side - 1), fill=COLORS[color]
            )
            image.save(train / f"{name}.png")
            (train / f"{name}.txt").write_text(item["prompt"])
            records.append(
                {
                    "image": f"train/{name}.png",
                    "prompt": item["prompt"],
                    "license": "MIT",
                    "source": "owned procedural geometry",
                }
            )
    # Holdout images are not exposed to the trainer. Prompts intentionally share
    # concepts; unseen seeds measure stochastic retention, not novel concepts.
    heldout = [
        spec(color, 112, seed)
        for color, seed in [
            ("red", 9401),
            ("blue", 9402),
            ("green", 9403),
            ("purple", 9404),
        ]
    ]
    config = {
        "model": "flux2-klein-4b",
        "model_path": str(ROOT / "models/flux2-klein-4b-4bit"),
        "data": str(train),
        "seed": 731,
        "steps": 4,
        "quantize": 4,
        "max_resolution": 256,
        "low_ram": True,
        "gradient_checkpointing": True,
        "cache_limit_gb": 4,
        "training_loop": {
            "num_epochs": 5,
            "batch_size": 1,
            "timestep_low": 0,
            "timestep_high": 4,
        },
        "optimizer": {"name": "AdamW", "learning_rate": 1e-5},
        "checkpoint": {
            "save_frequency": 12,
            "output_path": str(destination / "checkpoints"),
        },
        "lora_layers": {
            "targets": [
                {
                    "module_path": "transformer_blocks.{block}.attn.to_q",
                    "blocks": {"start": 0, "end": 1},
                    "rank": 4,
                }
            ]
        },
    }
    (destination / "train.json").write_text(json.dumps(config, indent=2))
    manifest = {
        "version": 1,
        "training_records": records,
        "heldout": heldout,
        "evaluation_scope": "same concepts, unseen fixed seeds; geometric conformity only, not perceptual quality or generalization",
        "criteria": {
            "foreground_area_fraction": [0.08, 0.4],
            "centroid_distance_from_center_max": 0.12,
            "foreground_bbox_aspect_ratio": [0.8, 1.25],
            "foreground_fill_fraction_min": 0.85,
            "dominant_requested_color_fraction_min": 0.85,
            "white_background_fraction_min": 0.55,
        },
        "criteria_frozen_before_inference": True,
        "provider_budget_usd": 0,
        "base_weights_immutable": True,
        "automatic_promotion": False,
        "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "files_sha256": {
            str(p.relative_to(destination)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(destination.rglob("*"))
            if p.is_file()
        },
    }
    (destination / "experiment.json").write_text(json.dumps(manifest, indent=2))
    return manifest


if __name__ == "__main__":
    destination = ROOT / "data/weight-experiments/image-geometry-v1"
    manifest = prepare(destination)
    print(
        json.dumps(
            {
                "directory": str(destination),
                "training_images": len(manifest["training_records"]),
                "heldout_seeds": len(manifest["heldout"]),
                "training_started": False,
            }
        )
    )
