#!/usr/bin/env python3
"""Owned counting fixtures. No weights, models or downloads are used."""

import hashlib
import json

from PIL import Image, ImageDraw
from prepare_image_weight_experiment import COLORS, ROOT


def prompt(color, count):
    return f"Exactly {count} separate solid {color} squares on a pure white background. Flat minimal geometric icons, spaced apart, no overlap. No text, shadows, outlines, gradients or other objects."


def prepare(target):
    target.mkdir(parents=True, exist_ok=False)
    train = target / "train"
    train.mkdir()
    records = []
    for color in ["red", "green", "blue"]:
        for count in range(2, 8):
            image = Image.new("RGB", (256, 256), "white")
            draw = ImageDraw.Draw(image)
            for i in range(count):
                x = 24 + (i % 3) * 80
                y = 24 + (i // 3) * 80
                draw.rectangle((x, y, x + 35, y + 35), fill=COLORS[color])
            name = f"{color}-{count}"
            image.save(train / f"{name}.png")
            (train / f"{name}.txt").write_text(prompt(color, count))
            records.append(
                {
                    "image": f"train/{name}.png",
                    "count": count,
                    "color": color,
                    "license": "MIT",
                    "source": "owned procedural geometry",
                }
            )
    original = json.loads(
        (ROOT / "data/weight-experiments/image-geometry-v1/train.json").read_text()
    )
    original["data"] = str(train)
    original["checkpoint"]["output_path"] = str(target / "checkpoints")
    (target / "train.json").write_text(json.dumps(original, indent=2))
    manifest = {
        "version": 1,
        "training_records": records,
        "heldout": [
            {
                "prompt": prompt("green", count),
                "count": count,
                "color": "green",
                "seed": 9500 + count,
                "width": 256,
                "height": 256,
            }
            for count in [4, 5, 6, 7]
        ],
        "evaluation_scope": "Exact count and component shape/color on known concepts with new fixed seeds; not general visual quality",
        "criteria": {
            "minimum_component_pixels": 64,
            "minimum_foreground_fraction": 0.02,
            "minimum_white_fraction": 0.55,
            "minimum_component_fill": 0.75,
            "component_aspect_ratio": [0.7, 1.43],
        },
        "provider_budget_usd": 0,
        "base_weights_immutable": True,
        "automatic_promotion": False,
        "files_sha256": {
            str(p.relative_to(target)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(target.rglob("*"))
            if p.is_file()
        },
    }
    (target / "experiment.json").write_text(json.dumps(manifest, indent=2))
    sources = [
        "scripts/prepare_image_count_experiment.py",
        "scripts/image_count_evaluation.py",
    ]
    (target / "preflight.json").write_text(
        json.dumps(
            {
                "source_sha256": {
                    f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest()
                    for f in sources
                }
            },
            indent=2,
        )
    )
    print(target)


if __name__ == "__main__":
    prepare(ROOT / "data/weight-experiments/image-count-v1")
