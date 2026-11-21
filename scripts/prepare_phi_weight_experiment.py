#!/usr/bin/env python3
"""Freeze owned raw Python-completion cases for an untrained Phi-1 baseline."""

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fixture():
    operations = [
        ("add", "+"),
        ("subtract", "-"),
        ("multiply", "*"),
        ("floor divide", "//"),
    ]
    result = {}
    for split, count in [("train", 40), ("valid", 8), ("test", 12)]:
        rows = []
        for i in range(count):
            verb, op = operations[i % len(operations)]
            name = f"{split}_operation_{i}"
            prompt = f"# Complete the function with a single return statement.\n# {verb.capitalize()} a and b.\ndef {name}(a, b):\n"
            rows.append(
                {
                    "company_id": "we-need-gents-lc",
                    "role": "backend-engineer",
                    "reviewed": True,
                    "source": "gents-owned-python-completion-v1",
                    "license": "MIT",
                    "messages": [
                        {"role": "user", "content": prompt},
                        {"role": "assistant", "content": f"    return a {op} b\n"},
                    ],
                }
            )
        result[split] = rows
    return result


if __name__ == "__main__":
    target = ROOT / "data/weight-experiments/phi-completion-v1"
    target.mkdir(parents=True, exist_ok=False)
    pins = {}
    for split, rows in fixture().items():
        payload = "".join(json.dumps(row) + "\n" for row in rows).encode()
        (target / (split + ".jsonl")).write_bytes(payload)
        pins[split] = hashlib.sha256(payload).hexdigest()
    (target / "protocol.json").write_text(
        json.dumps(
            {
                "base_model": "models/phi-1-mlx-4bit",
                "prompt_format": "completion-v1",
                "training_profile": "conservative-v1",
                "iterations": 100,
                "sequence_length": 256,
                "require_baseline_failure": True,
                "dataset_sha256": pins,
                "scope": "Exact raw code completion on synthetic same-template held-out function names; not general coding ability.",
                "provider_budget_usd": 0,
                "promote": False,
            },
            indent=2,
        )
        + "\n"
    )
    print(target)
