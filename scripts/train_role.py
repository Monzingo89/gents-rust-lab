#!/usr/bin/env python3
"""Offline MLX LoRA worker. Invoked by the harness while holding its GPU lease."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import time
from types import SimpleNamespace


def digest(path):
    result=hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda:stream.read(8*1024*1024),b""):
            result.update(block)
    return result.hexdigest()


def confined(root, path):
    path=Path(path)
    if not path.is_absolute():
        path=root/path
    if not path.absolute().is_relative_to(root):
        raise ValueError("path escapes scope")
    cursor=root
    for part in path.absolute().relative_to(root).parts:
        if part in (".",".."):
            raise ValueError("relative traversal rejected")
        cursor/=part
        if cursor.is_symlink():
            raise ValueError("training symlink rejected")
    return path


def samples(dataset, company, role, smoke=False):
    result={}
    fingerprints=set()
    for split,minimum in [("train",2 if smoke else 20),("valid",1 if smoke else 5),("test",1 if smoke else 5)]:
        path=dataset/f"{split}.jsonl"
        if path.is_symlink() or path.stat().st_size>10*1024*1024:
            raise ValueError("dataset must be a regular JSONL file under 10 MiB")
        rows=[]
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            item=json.loads(line)
            if item.get("company_id")!=company or item.get("role")!=role or item.get("reviewed") is not True:
                raise ValueError("every example needs matching company_id, role and reviewed=true")
            if not item.get("source") or not item.get("license") or "owner-evaluation" in item["source"]:
                raise ValueError("examples need provenance and training rights; owner evaluation is excluded")
            messages=item.get("messages",[])
            if len(messages)<2 or messages[-1].get("role")!="assistant" or not any(m.get("role")=="user" for m in messages):
                raise ValueError("expected reviewed user/assistant messages")
            if any(m.get("role") not in ("system","user","assistant") or not isinstance(m.get("content"),str) or not m["content"].strip() for m in messages):
                raise ValueError("invalid training message")
            # A held-out prompt must not also occur in train/validation, even with another answer.
            key=hashlib.sha256(json.dumps([m for m in messages if m["role"]!="assistant"],sort_keys=True).encode()).hexdigest()
            if key in fingerprints:
                raise ValueError("duplicate prompt across dataset splits")
            fingerprints.add(key)
            rows.append({"messages":messages})
        if len(rows)<minimum:
            raise ValueError(f"{split} needs at least {minimum} reviewed examples")
        result[split]=rows
    return result


def atomic_json(path, value):
    temporary=path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value,indent=2,allow_nan=False))
    temporary.replace(path)


def execute(manifest_path):
    manifest_path=Path(manifest_path).absolute()
    manifest=json.loads(manifest_path.read_text())
    from weight_training_profiles import profile
    training_profile=profile(manifest.get("training_profile","conservative-v1"))
    from weight_prompt_formats import datasets, inference_prompt, validate_format
    prompt_format=validate_format(manifest.get("prompt_format","chat-v1"))
    root=Path(manifest["repository_root"]).resolve()
    company=manifest["company_id"]
    if any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in company) or not company:
        raise ValueError("invalid company")
    company_root=confined(root,root/"data/companies"/company)
    work=confined(company_root,manifest_path.parent)
    dataset=confined(company_root,company_root/"training/datasets"/manifest["training_id"])
    model=confined(root/"models",root/manifest["base_model"])
    if not model.is_dir() or not list(model.glob("*.safetensors")):
        raise ValueError("training requires an MLX-compatible Safetensors directory, not a GGUF")
    for split,expected in manifest["dataset_sha256"].items():
        if split not in ("train","valid","test") or digest(dataset/f"{split}.jsonl")!=expected:
            raise ValueError("reviewed dataset changed after registration")
    rows=samples(dataset,company,manifest["role"],manifest.get("smoke",False))
    progress={"company_id":company,"training_id":manifest["training_id"],"role":manifest["role"],"phase":"loading","updated_at_ms":int(time.time()*1000)}
    atomic_json(work/"progress.json",progress)
    # Configure only fixed local behavior. No tracker, upload or remote model code.
    os.environ.update(HF_HUB_OFFLINE="1",TRANSFORMERS_OFFLINE="1",HF_HUB_DISABLE_TELEMETRY="1",WANDB_MODE="disabled",DO_NOT_TRACK="1")
    import mlx.core as mx
    from mlx_lm import load
    from mlx_lm.lora import CONFIG_DEFAULTS, train_model
    from mlx_lm.tuner.callbacks import TrainingCallback
    from mlx_lm.tuner.datasets import CacheDataset
    from mlx_lm.tuner.trainer import evaluate
    from mlx_lm.tuner.utils import load_adapters
    from mlx_lm import generate
    from mlx_lm.sample_utils import make_sampler
    from weight_task_evaluation import evaluate_tasks, compare
    started=time.monotonic()
    base_files={str(p.relative_to(model)):digest(p) for p in sorted(model.glob("*")) if p.suffix in (".safetensors",".json",".jinja")}
    base_fingerprint=hashlib.sha256(json.dumps(base_files,sort_keys=True).encode()).hexdigest()
    loaded,tokenizer=load(str(model),tokenizer_config={"trust_remote_code":False},trust_remote_code=False)
    parent=None
    if manifest.get("parent_training_id"):
        parent=confined(company_root,company_root/"training/runs"/manifest["parent_training_id"])
        parent_metrics=json.loads((parent/"metrics.json").read_text())
        if parent_metrics.get("training_profile",profile()) != training_profile:
            raise ValueError("parent adapter profile differs; start a separate base experiment")
        if parent_metrics["base_fingerprint"]!=base_fingerprint or digest(parent/"adapter/adapters.safetensors")!=manifest["parent_adapter_sha256"]:
            raise ValueError("incumbent adapter lineage changed")
        load_adapters(loaded,str(parent/"adapter"))
    sets=datasets(rows,tokenizer,prompt_format)
    # Reject truncation rather than accidentally train on an empty/partial completion.
    for split,items in rows.items():
        for index in range(len(items)):
            tokens,offset=sets[split].process(items[index])
            if len(tokens)>manifest["sequence_length"] or offset>=len(tokens)-1:
                raise ValueError("example exceeds sequence length or has no completion tokens; shorten it")
    def loss():
        value=evaluate(loaded,CacheDataset(sets["test"]),1,-1,max_seq_length=manifest["sequence_length"])
        if not math.isfinite(value):
            raise ValueError("non-finite evaluation loss")
        return value
    progress["phase"]="baseline_evaluation"
    atomic_json(work/"progress.json",progress)
    baseline=loss()
    def task_check():
        def infer(messages):
            mx.random.seed(17)
            prompt=inference_prompt(tokenizer,messages,prompt_format)
            if prompt_format=="completion-v1":
                prompt=tokenizer.encode(prompt,add_special_tokens=False)
            return generate(loaded,tokenizer,prompt=prompt,max_tokens=96,sampler=make_sampler(temp=0),verbose=False)
        return evaluate_tasks(rows["test"],infer)
    before_tasks=task_check()
    atomic_json(work/"task-baseline.json",before_tasks)
    if manifest.get("require_baseline_failure") is True and before_tasks["passed"] == before_tasks["total"]:
        atomic_json(work/"experiment-decision.json",{
            "decision":"skip_training_saturated_benchmark", "weights_trained":False,
            "provider_cost_usd":0, "promotion_authorized":False})
        raise ValueError("Training skipped: all frozen baseline tasks passed; harder benchmark required")
    if parent:
        import gc
        del loaded
        gc.collect()
        mx.clear_cache()
        loaded,tokenizer=load(str(model),tokenizer_config={"trust_remote_code":False},trust_remote_code=False)
    class Progress(TrainingCallback):
        def on_train_loss_report(self, info):
            progress.update(phase="training",metrics=info,updated_at_ms=int(time.time()*1000))
            atomic_json(work/"progress.json",progress)
        def on_val_loss_report(self, info):
            progress.update(validation=info,updated_at_ms=int(time.time()*1000))
            atomic_json(work/"progress.json",progress)
    options=dict(CONFIG_DEFAULTS)
    options.update(model=str(model),data=str(dataset),train=True,fine_tune_type="lora",num_layers=training_profile["layers"],
        batch_size=1,iters=manifest["iterations"],max_seq_length=manifest["sequence_length"],
        val_batches=1,steps_per_report=1,steps_per_eval=25,save_every=25,seed=17,
        learning_rate=training_profile["learning_rate"],grad_checkpoint=True,mask_prompt=True,
        lora_parameters={"rank":training_profile["rank"],"dropout":0.0,"scale":training_profile["scale"]},adapter_path=str(work/"adapter"),
        report_to=None,trust_remote_code=False)
    if parent:
        options["resume_adapter_file"]=str(parent/"adapter/adapters.safetensors")
    train_model(SimpleNamespace(**options),loaded,sets["train"],sets["valid"],Progress())
    progress.update(phase="candidate_evaluation",updated_at_ms=int(time.time()*1000))
    atomic_json(work/"progress.json",progress)
    adapted=loss()
    after_tasks=task_check()
    atomic_json(work/"task-candidate.json",after_tasks)
    # Exercise the bytes that deployment would actually load, not just the
    # in-memory module left by the optimizer. Release it before loading again.
    import gc
    del loaded
    gc.collect()
    mx.clear_cache()
    progress.update(phase="adapter_reload_evaluation",updated_at_ms=int(time.time()*1000))
    atomic_json(work/"progress.json",progress)
    loaded,tokenizer=load(str(model),tokenizer_config={"trust_remote_code":False},trust_remote_code=False)
    load_adapters(loaded,str(work/"adapter"))
    reloaded_tasks=task_check()
    atomic_json(work/"task-reloaded.json",reloaded_tasks)
    reload_matches=all(a["output"]==b["output"] for a,b in zip(after_tasks["cases"],reloaded_tasks["cases"]))
    if not reload_matches:
        raise ValueError("persisted adapter outputs differ from in-memory candidate; promotion denied")
    comparison=compare(before_tasks,after_tasks)
    atomic_json(work/"task-comparison.json",comparison)
    if any(digest(model/name)!=expected for name,expected in base_files.items()):
        raise ValueError("base model changed during experiment; candidate rejected")
    adapter=work/"adapter/adapters.safetensors"
    metrics={"company_id":company,"training_id":manifest["training_id"],"role":manifest["role"],
        "weights_trained":True,"method":f"LoRA rank {training_profile['rank']}, last {training_profile['layers']} transformer layers",
        "training_profile":training_profile, "base_fingerprint":base_fingerprint,
        "base_files":base_files,"adapter_sha256":digest(adapter),"baseline_test_loss":baseline,
        "adapter_test_loss":adapted,"loss_improved":adapted<baseline,"dataset_counts":{k:len(v) for k,v in rows.items()},
        "task_comparison":comparison,
        "persisted_adapter_reload_matches":reload_matches,
        "iterations":manifest["iterations"],"elapsed_seconds":round(time.monotonic()-started,2),
        "peak_memory_gb":mx.get_peak_memory()/1e9,"smoke":manifest.get("smoke",False),
        "promotion":"requires independent task/regression/privacy evaluation and operator review",
        "speed_improvement_proven":False,"base_weights_modified":False}
    atomic_json(work/"metrics.json",metrics)
    progress.update(phase="candidate",updated_at_ms=int(time.time()*1000),metrics=metrics)
    atomic_json(work/"progress.json",progress)


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--manifest",required=True)
    execute(parser.parse_args().manifest)
