"""Deterministically convert the supplied brand PNG to a multi-resolution ICO.

Use the installed media environment: .venv-media/bin/python scripts/build-brand-icon.py
No model calls; the original PNG is preserved.
"""

import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "assets/brand/gents-icon-transparent.png"
output = source.with_suffix(".ico")
with Image.open(source) as image:
    image.convert("RGBA").save(
        output,
        format="ICO",
        sizes=[
            (16, 16),
            (24, 24),
            (32, 32),
            (48, 48),
            (64, 64),
            (128, 128),
            (256, 256),
        ],
    )
shutil.copyfile(output, ROOT / "web/assets" / output.name)
shutil.copyfile(output, ROOT / "src-tauri/icons/icon.ico")
print(output)
