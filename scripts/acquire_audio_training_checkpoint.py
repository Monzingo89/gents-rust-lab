#!/usr/bin/env python3
"""Download pinned public training bytes into quarantine; never deserialize them."""

import hashlib
import json
import os
import shutil
import ssl
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    os.umask(0o077)
    config = json.loads((ROOT / "config/audio-training-source.json").read_text())
    target = ROOT / "data/model-quarantine/piper-ljspeech-training"
    target.mkdir(parents=True, exist_ok=False)
    if shutil.disk_usage(target).free < 3 * 1024**3:
        raise ValueError("insufficient quarantine space")
    context = ssl.create_default_context()
    records = []
    for entry in config["metadata"] + [config["checkpoint"]]:
        url = f"https://huggingface.co/datasets/{config['repo']}/resolve/{config['revision']}/{config['directory']}/{entry['name']}"
        path = target / entry["name"]
        temporary = path.with_suffix(path.suffix + ".partial")
        sha = hashlib.sha256()
        blob = hashlib.sha1(f"blob {entry['bytes']}\0".encode())
        count = 0
        with (
            urllib.request.urlopen(url, context=context, timeout=60) as response,
            temporary.open("xb") as output,
        ):
            if not response.url.startswith("https://"):
                raise ValueError("non-TLS redirect rejected")
            while block := response.read(1024 * 1024):
                count += len(block)
                if count > entry["bytes"]:
                    raise ValueError("download exceeded pinned size")
                output.write(block)
                sha.update(block)
                blob.update(block)
            output.flush()
            os.fsync(output.fileno())
        if (
            count != entry["bytes"]
            or ("sha256" in entry and sha.hexdigest() != entry["sha256"])
            or ("git_blob_sha1" in entry and blob.hexdigest() != entry["git_blob_sha1"])
        ):
            raise ValueError("download differs from frozen source pin")
        temporary.rename(path)
        records.append(
            {"file": entry["name"], "bytes": count, "sha256": sha.hexdigest()}
        )
        print(json.dumps(records[-1]), flush=True)
    (target / "acquisition.json").write_text(
        json.dumps(
            {
                "source": config,
                "files": records,
                "deserialized": False,
                "admitted": False,
                "provider_cost_usd": 0,
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
