#!/usr/bin/env python3
"""Verify and score frozen native image baseline; no model inference."""
import hashlib
import json
from pathlib import Path
from PIL import Image
from image_geometry_evaluation import measure

ROOT=Path(__file__).resolve().parents[1]


def evaluate(folder):
    counting=folder.name=="image-count-v1"
    if folder.name not in {"image-count-v1","image-geometry-v1"}:
        raise ValueError("unknown image experiment")
    baseline=folder/"baseline"
    frozen=(baseline/"frozen-experiment.json").read_bytes()
    manifest=json.loads(frozen)
    receipts=json.loads((baseline/"receipts.json").read_text())
    if receipts["complete"] is not True or receipts["experiment_sha256"]!=hashlib.sha256(frozen).hexdigest():
        raise ValueError("baseline incomplete or frozen experiment changed")
    if frozen!=(folder/"experiment.json").read_bytes():
        raise ValueError("experiment changed after generation")
    if len(receipts["receipts"])!=len(manifest["heldout"]):
        raise ValueError("case counts differ")
    preflight=json.loads((folder/"preflight.json").read_text())
    scorer_name="scripts/image_count_evaluation.py" if counting else "scripts/image_geometry_evaluation.py"
    scorer=ROOT/scorer_name
    if hashlib.sha256(scorer.read_bytes()).hexdigest()!=preflight["source_sha256"][scorer_name]:
        raise ValueError("scorer changed since preflight")
    results=[]
    for index,(case,receipt) in enumerate(zip(manifest["heldout"],receipts["receipts"])):
        name=f"case-{index}.png"
        if receipt["case"]!=index or receipt["file"]!=name:
            raise ValueError("case identity changed")
        path=baseline/name
        if path.is_symlink() or path.stat().st_size>8*1024**2:
            raise ValueError("unsafe image artifact")
        digest=hashlib.sha256(path.read_bytes()).hexdigest()
        if digest!=receipt["image_sha256"] or digest!=receipt["native_receipt"]["value"]["artifact"]["sha256"]:
            raise ValueError("image differs from native receipt")
        with Image.open(path) as image:
            if counting:
                from image_count_evaluation import measure as count_measure
                result=count_measure(image,case,manifest["criteria"])
            else:
                result=measure(image,case["color"],manifest["criteria"])
        results.append({"index":index,"color":case["color"],"seed":case["seed"],
                        "image_sha256":digest,"elapsed_ms":receipt["elapsed_ms"],**result})
    passed=sum(r["passed"] for r in results)
    return {"scope":manifest["evaluation_scope"],"cases":results,"passed":passed,"total":len(results),
            "provider_cost_usd":0,"weights_modified":False,
            "decision":"skip_training_saturated_baseline" if passed==len(results) else "baseline_has_failures_candidate_training_not_yet_run"}


if __name__=="__main__":
    import argparse
    parser=argparse.ArgumentParser()
    parser.add_argument("--experiment",choices=["image-geometry-v1","image-count-v1"],default="image-geometry-v1")
    folder=ROOT/"data/weight-experiments"/parser.parse_args().experiment
    report=evaluate(folder)
    (folder/"baseline-evaluation.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))
