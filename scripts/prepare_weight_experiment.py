#!/usr/bin/env python3
"""Create a fixed synthetic extraction experiment; never starts or promotes training."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fixture():
    def row(prompt, answer):
        return {"company_id":"we-need-gents-lc", "role":"backend-engineer", "reviewed":True,
                "source":"gents-owned-synthetic-extraction-v1", "license":"MIT",
                "messages":[{"role":"user","content":prompt}, {"role":"assistant","content":answer}]}
    def extraction(name, items):
        prompt = ('Return only a JSON object with keys "name" (string) and "items" (array of strings). '
                  'Extract the following synthetic record exactly. No Markdown or extra fields.\n'
                  f'Name: {name}\nItems: {", ".join(items)}')
        return row(prompt, json.dumps({"name":name,"items":items}, separators=(",", ":")))
    train = [extraction(f"sample-{i}", [f"part-{i}", f"unit-{i+1}"]) for i in range(20)]
    valid = [extraction(f"validation-{i}", [f"piece-{i}", f"spare-{i+2}"]) for i in range(5)]
    test = [extraction("Cedar", ["bolt", "washer"]), extraction("Harbor", ["cable", "socket", "clip"]),
            extraction("Orchid", ["red cap"]), extraction("Team Delta", ["north wing", "south wing"]),
            row("Reply with only the integer result of 7 + 8.", "15"),
            row("Return only the lowercase English word for the opposite of hot.", "cold")]
    return {"train":train,"valid":valid,"test":test}


if __name__ == "__main__":
    destination = ROOT / "data/weight-experiments/extraction-v1"
    destination.mkdir(parents=True, exist_ok=False)
    for split, rows in fixture().items():
        (destination / f"{split}.jsonl").write_text("".join(json.dumps(row)+"\n" for row in rows))
    print(destination)
