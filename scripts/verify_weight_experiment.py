#!/usr/bin/env python3
"""Recompute local experiment evidence without inference or weight mutation."""
import argparse
import hashlib
import json
from pathlib import Path
from train_role import digest
from weight_task_evaluation import compare,score


def verify(work):
    read=lambda name:json.loads((work/name).read_text())
    manifest,metrics,experiment=read("manifest.json"),read("metrics.json"),read("experiment.json")
    root=Path(manifest["repository_root"])
    def require(condition, message):
        if not condition:
            raise ValueError(message)
    for source,expected in experiment["source_sha256"].items():
        archived=work/"source-snapshot"/source
        actual=archived if archived.is_file() else root/source
        require(digest(actual)==expected,"experiment source changed: "+source)
    for name,expected in metrics["base_files"].items():
        require(digest(root/manifest["base_model"]/name)==expected,"base changed: "+name)
    require(digest(work/"adapter/adapters.safetensors")==metrics["adapter_sha256"],"adapter changed")
    dataset=root/"data/companies"/manifest["company_id"]/"training/datasets"/manifest["training_id"]
    for split,expected in manifest["dataset_sha256"].items():
        require(digest(dataset/f"{split}.jsonl")==expected,"dataset changed")
    rows=[json.loads(line) for line in (dataset/"test.jsonl").read_text().splitlines() if line.strip()]
    before,after=read("task-baseline.json"),read("task-candidate.json")
    reloaded=read("task-reloaded.json") if metrics.get("persisted_adapter_reload_matches") is True else None
    for result in ([before,after,reloaded] if reloaded else [before,after]):
        require(len(result["cases"])==len(rows)==result["total"],"case count mismatch")
        for row,case in zip(rows,result["cases"]):
            prompt=row["messages"][:-1]
            require(hashlib.sha256(json.dumps(prompt,sort_keys=True).encode()).hexdigest()==case["prompt_sha256"],"prompt changed")
            require(row["messages"][-1]["content"]==case["expected"],"oracle changed")
            require(score(case["output"],case["expected"])==case["passed"],"incorrect score")
        require(sum(case["passed"] for case in result["cases"])==result["passed"],"incorrect total")
    if reloaded:
        require([c["output"] for c in after["cases"]]==[c["output"] for c in reloaded["cases"]],"reload output mismatch")
    comparison=compare(before,after)
    require(comparison==read("task-comparison.json")==metrics["task_comparison"],"comparison mismatch")
    return {"training_id":manifest["training_id"],"verified":True,
            "scope":"Local hashes, frozen prompts/oracles, persisted output scoring; not tamper-proof native execution attestation",
            "task_comparison":comparison,"adapter_sha256":metrics["adapter_sha256"],
            "persisted_adapter_reload_matches":bool(reloaded),
            "iterations":metrics["iterations"],"elapsed_seconds":metrics["elapsed_seconds"],
            "peak_memory_gb":metrics["peak_memory_gb"],"provider_cost_usd":0,"promoted":False}


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("work",type=Path)
    print(json.dumps(verify(parser.parse_args().work),indent=2))
