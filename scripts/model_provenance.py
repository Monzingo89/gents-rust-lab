#!/usr/bin/env python3
"""Verify pinned model artifacts and retain small publisher/license evidence.

No weights are downloaded; no repository code or hosted inference is executed.
Role qualification, legal review and production admission are separate gates.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
ROSTER = ROOT / "config/enterprise-model-roster.json"


def local_path(root, relative, prefix):
    path = Path(relative)
    if path.is_absolute() or not path.parts or path.parts[0] != prefix or ".." in path.parts:
        raise ValueError("path is outside the approved local scope")
    cursor = root
    for part in path.parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError("symlink in provenance path")
    return cursor


def validate_roster(value):
    models = value.get("models", [])
    if value.get("version") != 1 or not 10 <= len(models) <= 32:
        raise ValueError("roster must name 10..32 candidates")
    for key in ("id", "publisher", "path", "sha256"):
        if len({m[key] for m in models}) != len(models):
            raise ValueError("aliases, duplicate paths or weight hashes do not add models")
    for model in models:
        if not re.fullmatch(r"[a-z][a-z0-9-]{0,40}", model["id"]):
            raise ValueError("invalid model identity")
        for field in ("publisher", "source_repo"):
            if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", model[field]):
                raise ValueError("invalid publisher/source repository")
        for field, length in (("source_revision", 40), ("sha256", 64)):
            if not re.fullmatch(r"[0-9a-f]{%d}" % length, model[field]):
                raise ValueError("model source must be immutable and hash-pinned")
        if model["context_tokens"] != 8192 or model["license"] not in {"apache-2.0", "mit"}:
            raise ValueError("unreviewed evaluation context/license family")
        local_path(ROOT, model["path"], "models")
    return models


def cli(*arguments):
    env = {k: v for k, v in os.environ.items() if k in {"PATH", "HOME", "USER", "LANG", "TMPDIR"}}
    env["HF_HUB_DISABLE_IMPLICIT_TOKEN"] = "1"
    result = subprocess.run(["hf", *arguments], capture_output=True, text=True, timeout=180, env=env)
    if result.returncode:
        # Do not echo provider diagnostics or inherited credentials into reports.
        raise RuntimeError("public Hugging Face metadata request failed")
    if len(result.stdout.encode()) > 16_000_000:
        raise ValueError("remote metadata exceeds bounded response size")
    return json.loads(result.stdout)


def file_sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(4 * 1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def private_json(path, value):
    if path.is_symlink():
        raise ValueError("report symlink rejected")
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, prefix=".provenance-", delete=False) as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")
        temporary = stream.name
    os.replace(temporary, path)


def documents(repo, revision, folder, files):
    local_path(ROOT, str(folder.relative_to(ROOT)), "data")
    selected = [row["path"] for row in files if row["path"] in {"README.md", "LICENSE", "LICENSE.txt", "LICENSE.md"}
                and 0 <= row.get("size", 1_000_001) <= 1_000_000]
    if "README.md" not in selected:
        raise ValueError("pinned model card is missing or too large")
    folder.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name in selected:
        local_path(ROOT, str((folder / name).relative_to(ROOT)), "data")
    cli("download", repo, *selected, "--revision", revision, "--local-dir", str(folder), "--format", "json")
    evidence = []
    for name in selected:
        path = folder / name
        if path.is_symlink() or path.stat().st_size > 1_000_000:
            raise ValueError("untrusted document destination")
        evidence.append({"filename": name, "sha256": file_sha(path),
                         "source": f"https://huggingface.co/{repo}/blob/{revision}/{name}"})
    return evidence


def verify(model, folder):
    path = local_path(ROOT, model["path"], "models")
    if not path.is_file() or path.suffix != ".gguf":
        raise ValueError("expected an existing local GGUF artifact")
    files = cli("models", "list", model["source_repo"], "--revision", model["source_revision"], "--format", "json")
    entry = next(row for row in files if row["path"] == path.name)
    remote_sha = entry.get("lfs", {}).get("sha256")
    actual_sha = file_sha(path)
    if remote_sha != actual_sha or actual_sha != model["sha256"] or entry["size"] != path.stat().st_size:
        raise ValueError("pinned remote/local/inventory weight identity mismatch")
    source_folder = folder / ("source-" + model["source_revision"])
    source_docs = documents(model["source_repo"], model["source_revision"], source_folder, files)
    publisher_revision = cli("models", "info", model["publisher"], "--expand", "sha", "--format", "json")["sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", publisher_revision):
        raise ValueError("publisher revision is not immutable")
    publisher_files = cli("models", "list", model["publisher"], "--revision", publisher_revision, "--format", "json")
    publisher_folder = folder / ("publisher-" + publisher_revision)
    publisher_docs = documents(model["publisher"], publisher_revision, publisher_folder, publisher_files)
    card = (publisher_folder / "README.md").read_text()
    declared = re.search(r"(?m)^license:\s*['\"]?([a-zA-Z0-9.-]+)", card)
    if not declared or declared.group(1).lower() != model["license"]:
        raise ValueError("publisher license declaration differs from the evaluation roster")
    source_card = (source_folder / "README.md").read_text()
    linked = model["publisher"] in source_card or model["source_repo"] == model["publisher"] + "-GGUF"
    if not linked:
        raise ValueError("quantizer card does not establish the configured publisher link")
    return {"id": model["id"], "publisher": model["publisher"], "publisher_revision": publisher_revision,
            "source_repo": model["source_repo"], "source_revision": model["source_revision"],
            "path": model["path"], "bytes": path.stat().st_size, "sha256": actual_sha,
            "weight_identity_verified": True, "publisher_link_found": True,
            "declared_license": model["license"], "source_documents": source_docs, "publisher_documents": publisher_docs,
            "role_qualified": False, "production_admitted": False,
            "scope": "artifact and declared-license evidence only; not legal approval, role qualification or compliance certification"}


def verify_cached(model, candidates):
    """Artifact provenance is independent of its new role; role tests are not."""
    for previous_path, proof in candidates:
        if proof.get("status") != "PASS" or any(proof.get(k) != model[k] for k in
                ("publisher", "source_repo", "source_revision", "path", "sha256")):
            continue
        if proof.get("declared_license") != model["license"] or not proof.get("weight_identity_verified"):
            continue
        path=local_path(ROOT,model["path"],"models")
        if not path.is_file() or path.stat().st_size != proof["bytes"] or file_sha(path) != model["sha256"]:
            raise ValueError("cached provenance weight changed")
        base=Path(proof.get("artifact_evidence_root",str(previous_path.parent)))
        local_path(ROOT,str(base.relative_to(ROOT)),"data")
        for prefix,revision,documents_key,repo in (
                ("source",model["source_revision"],"source_documents",model["source_repo"]),
                ("publisher",proof["publisher_revision"],"publisher_documents",model["publisher"])):
            if not re.fullmatch(r"[0-9a-f]{40}",revision):
                raise ValueError("cached document revision is not pinned")
            docs=proof[documents_key]
            if not any(row["filename"]=="README.md" for row in docs):
                raise ValueError("cached model card missing")
            for row in docs:
                name=row["filename"]
                if name not in {"README.md","LICENSE","LICENSE.md","LICENSE.txt"}:
                    raise ValueError("cached document filename rejected")
                document=local_path(ROOT,str((base/f"{prefix}-{revision}"/name).relative_to(ROOT)),"data")
                if row["source"] != f"https://huggingface.co/{repo}/blob/{revision}/{name}" or file_sha(document)!=row["sha256"]:
                    raise ValueError("cached document integrity failed")
        return {**proof,"id":model["id"],"artifact_evidence_root":str(base),
                "cached_provenance_sha256":file_sha(previous_path),"cached_provenance_evidence":str(previous_path),"revalidated_offline":True,
                "role_qualified":False,"production_admitted":False}
    raise ValueError("no matching pinned artifact provenance; offline reassignment denied")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", action="append", help="Verify only named roster IDs")
    parser.add_argument("--reuse-verified",action="store_true",help="Rehash existing weights/cards offline for a role reassignment; never reuses role qualification")
    args = parser.parse_args()
    models = validate_roster(json.loads(ROSTER.read_text()))
    if args.model and set(args.model) - {m["id"] for m in models}:
        raise ValueError("unknown roster model")
    folder = local_path(ROOT, "data/model-registry/provenance", "data")
    folder.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(folder, 0o700)
    history = local_path(ROOT, "data/model-registry/provenance/reports", "data")
    history.mkdir(mode=0o700, exist_ok=True)
    previous = folder / "report.json"
    if previous.is_file() and not previous.is_symlink():
        private_json(history / (file_sha(previous) + ".json"), json.loads(previous.read_text()))
    started = time.monotonic()
    results = []
    cached=[]
    if args.reuse_verified:
        for path in folder.glob("*/result.json"):
            local_path(ROOT,str(path.relative_to(ROOT)),"data")
            proof=json.loads(path.read_text())
            snapshot=history/("offline-source-"+uuid.uuid4().hex+".json")
            private_json(snapshot,proof)
            cached.append((snapshot,{**proof,"artifact_evidence_root":proof.get("artifact_evidence_root",str(path.parent))}))
    for model in models:
        if args.model and model["id"] not in args.model:
            continue
        destination = local_path(ROOT, f"data/model-registry/provenance/{model['id']}", "data")
        destination.mkdir(mode=0o700, exist_ok=True)
        print(json.dumps({"model": model["id"], "status": "checking"}), flush=True)
        try:
            verified=verify_cached(model,cached) if args.reuse_verified else verify(model,destination)
            result = {"status": "PASS", **verified}
        except (ValueError, RuntimeError, OSError, StopIteration, KeyError, subprocess.SubprocessError) as error:
            result = {"id": model["id"], "status": "FAIL", "error_type": type(error).__name__,
                      "reason": str(error)[:200] if isinstance(error, (ValueError, RuntimeError)) else "See bounded metadata/checksum stage",
                      "weight_identity_verified": False, "role_qualified": False, "production_admitted": False}
        existing=destination/"result.json"
        if existing.is_file() and not existing.is_symlink():
            private_json(history/(file_sha(existing)+".json"),json.loads(existing.read_text()))
        private_json(existing, result)
        results.append(result)
        print(json.dumps({"model": model["id"], "status": result["status"]}), flush=True)
    report = {"version": 1, "roster_sha256": file_sha(ROSTER), "models": results,
              "verified_publishers": len({m["publisher"] for m in results if m["status"] == "PASS"}),
              "role_qualified_models": 0, "production_admitted_models": 0,
              "elapsed_seconds": round(time.monotonic() - started, 2)}
    private_json(folder / "report.json", report)
    private_json(history / (uuid.uuid4().hex + ".json"), report)
    print(json.dumps({"report": str(folder / "report.json"), "verified": report["verified_publishers"]}), flush=True)


if __name__ == "__main__":
    main()
