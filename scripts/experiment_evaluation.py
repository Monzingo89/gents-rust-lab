"""Trusted-host evaluation of submitted source; never promotion or model authority.

The native adapter owns the Rust dialect and OS isolation. This module binds its
snapshot result to a frozen owner plan and canonical proposal custody evidence.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess

import local_sqlite as sqlite3


def sha(body):
    return hashlib.sha256(body).hexdigest()


def executable_pin(path):
    path = Path(path)
    if not path.is_absolute() or path.is_symlink():
        raise ValueError("ordinary absolute evaluator executable required")
    path = path.resolve(strict=True)
    with path.open("rb") as stream:
        before = os.fstat(stream.fileno())
        if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_uid != os.getuid()
                or before.st_mode & 0o022 or not before.st_mode & 0o100 or before.st_size > 512 * 1024 * 1024):
            raise ValueError("bounded owner-controlled executable required")
        digest = hashlib.sha256()
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
        after = os.fstat(stream.fileno())
    identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
    if identity(before) != identity(after):
        raise ValueError("executable changed while hashing")
    return {"path": str(path), "sha256": digest.hexdigest()}


def run_native(runtime, request):
    for key in ("adapter", "compiler"):
        if executable_pin(runtime[key]["path"]) != runtime[key]:
            raise ValueError("frozen evaluator executable changed")
    wire = json.dumps(request, ensure_ascii=False, separators=(",", ":")).encode()
    if len(wire) > 49152:
        raise ValueError("native evaluator request exceeds frame bound")
    result = subprocess.run([runtime["adapter"]["path"]], input=wire, capture_output=True,
        timeout=80, cwd="/", env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "GENTS_RUSTC": runtime["compiler"]["path"]})
    if len(result.stdout) > 131072 or len(result.stderr) > 16384 or result.returncode not in (0, 1):
        raise ValueError("native evaluator did not return a bounded terminal protocol response")
    value = json.loads(result.stdout)
    if (type(value.get("passed")) is not bool or value.get("source_sha256") != request["source_sha256"]
            or value.get("tests_sha256") != request["tests_sha256"] or value.get("promotion_authorized") is not False
            or type(value.get("provider_cost_usd")) is not int or value["provider_cost_usd"] != 0
            or value.get("stage") not in ("admission", "compile_candidate", "compile_acceptance", "acceptance")
            or (result.returncode == 0) != value["passed"]):
        raise ValueError("native evaluator result identity or outcome mismatch")
    if value["passed"] and (value["stage"] != "acceptance" or value.get("diagnostics", {}).get("passed") is not True
            or type(value.get("tests_count")) is not int or not 1 <= value["tests_count"] <= 16):
        raise ValueError("native evaluator PASS lacks actual frozen acceptance execution")
    return value


def freeze(root, descriptor, trial, read_file):
    if (not isinstance(descriptor, dict) or set(descriptor) != {"kind", "tests_path", "tests_sha256"}
            or descriptor["kind"] != "rust_acceptance" or not isinstance(descriptor["tests_path"], str)
            or not descriptor["tests_path"].startswith("data/experiment-inputs/")):
        raise ValueError("exact owner-frozen Rust acceptance descriptor required")
    candidate = trial.get("candidate_workflow", {})
    if (candidate.get("actor") != trial["actor"] or not isinstance(candidate.get("path"), str)
            or not candidate["path"].endswith(".rs") or type(candidate.get("max_bytes")) is not int
            or not 1 <= candidate["max_bytes"] <= 8192):
        raise ValueError("Rust evaluation requires a bounded typed source candidate")
    tests = read_file(root, descriptor["tests_path"], 8192)
    if sha(tests) != descriptor["tests_sha256"]:
        raise ValueError("owner acceptance hash mismatch")
    tests = tests.decode("utf-8")
    if platform.system() != "Darwin":
        raise ValueError("isolated Rust evaluation requires the supported macOS adapter")
    compiler = Path(os.environ.get("GENTS_RUSTC", Path.home() / ".rustup/toolchains" /
        ("stable-" + platform.machine().replace("arm64", "aarch64") + "-apple-darwin") / "bin/rustc"))
    runtime = {"adapter": executable_pin(root / "target/debug/gents-rust-component"),
               "compiler": executable_pin(compiler)}
    # Native parser preflight only. It runs no candidate or acceptance code.
    checker = executable_pin(root / "target/debug/gents-rust-acceptance-check")
    parsed = subprocess.run([checker["path"]], input=tests.encode(), capture_output=True,
                            timeout=5, env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
    if parsed.returncode != 0 or len(parsed.stdout) > 16384:
        raise ValueError("frozen acceptance rejected by native parser preflight")
    fact = json.loads(parsed.stdout)
    if fact.get("passed") is not True or fact.get("source_sha256") != descriptor["tests_sha256"]:
        raise ValueError("frozen acceptance parser identity mismatch")
    return {**descriptor, "frozen_tests": tests, "runtime": runtime, "parser_preflight": fact,
            "parser_executable": checker}


def custody(root, company, run, trial, read_file):
    """Read-only reconciliation; native proposal state already verified each page."""
    path = root / "data/companies" / company / "brain.sqlite3"
    checked = root
    for part in path.relative_to(root).parts:
        checked /= part
        if checked.is_symlink():
            raise ValueError("linked company evidence rejected")
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        db.execute("BEGIN")
        if db.execute("SELECT owner FROM company").fetchall() != [("local-developer",)]:
            raise ValueError("local owner identity mismatch")
        intake, contract, approval, jobstate = db.execute(
            "SELECT intake,digest,approval,state FROM jobs WHERE run_id=?", (run,)).fetchone()
        approved = json.loads(approval)
        if (sha(intake.encode()) != contract or approved.get("sha256") != contract
                or approved.get("owner") != "local-developer" or approved.get("type") != "local_operator_approval"
                or jobstate != "candidate_submitted"):
            raise ValueError("candidate lacks current owner-approved submission")
        frozen_intake = json.loads(intake)
        if (frozen_intake.get("candidate_workflow") != trial["candidate_workflow"]
                or frozen_intake.get("idea") != trial["task"] or frozen_intake.get("acceptance") != "\n".join(trial["criteria"])
                or frozen_intake.get("candidate_feedback") != trial.get("candidate_feedback")):
            raise ValueError("candidate intake differs from frozen experiment")
        revision, manifest = db.execute("SELECT revision,manifest FROM run_governance WHERE run_id=?", (run,)).fetchone()
        policy = json.loads(manifest)
        tools = policy.get("grants", {}).get(trial["actor"], {}).get("tools", [])
        if policy.get("revision") != revision or not ("files" in tools or {"files:read", "files:write"} <= set(tools)):
            raise ValueError("candidate actor permissions were revoked")
        saved_contract, epoch, encoded, checksum = db.execute(
            "SELECT contract,revision,state,state_sha256 FROM candidate_workflows WHERE run_id=?", (run,)).fetchone()
        if saved_contract != contract or epoch != revision or sha(encoded.encode()) != checksum:
            raise ValueError("candidate checkpoint identity mismatch")
        state = json.loads(encoded)
        relative = "data/companies/" + company + "/runs/" + run + "/workspace/" + trial["candidate_workflow"]["path"]
        source = read_file(root, relative, 8192)
        if (state.get("stage") != "submitted" or state.get("source_sha256") != sha(source)
                or state.get("source") != source.decode("utf-8") or state.get("offset") != len(source)
                or not isinstance(state.get("write_receipt"), dict) or len(state.get("read_receipts", [])) != 1):
            raise ValueError("candidate lacks exact confirmed source write and full readback")
        receipts = [state["write_receipt"], *state["read_receipts"]]
        write_args = {"op": "write", "path": trial["candidate_workflow"]["path"], "content": source.decode()}
        read_args = {"op": "read", "path": trial["candidate_workflow"]["path"], "offset_bytes": 0, "max_bytes": 8192}
        sequences = []
        for receipt, arguments in zip(receipts, (write_args, read_args)):
            expected = sha(json.dumps(arguments, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode())
            if (receipt.get("actor") != trial["actor"] or receipt.get("phase") != "worker"
                    or receipt.get("tool") != "files" or receipt.get("policy_revision") != revision
                    or receipt.get("status") != "completed" or receipt.get("arguments_sha256") != expected):
                raise ValueError("candidate receipt scope or argument identity mismatch")
            matches = [seq for seq, body in db.execute("SELECT seq,body FROM audit WHERE run_id=? AND kind='mcp_tool_call'", (run,))
                       if json.loads(body) == receipt]
            if not matches:
                raise ValueError("candidate receipt absent from canonical journal")
            sequences.append(min(matches))
    return source.decode(), {"intake_sha256": contract, "policy_revision": revision,
                             "source_sha256": sha(source), "native_audit_sequences": sequences}


def evaluate(root, company, run, trial, read_file):
    spec = trial["evaluation"]
    source, provenance = custody(root, company, run, trial, read_file)
    tests = spec["frozen_tests"]
    if sha(tests.encode()) != spec["tests_sha256"]:
        raise ValueError("frozen acceptance snapshot changed")
    result = run_native(spec["runtime"], {"op": "verify", "source": source,
        "source_sha256": provenance["source_sha256"], "tests": tests, "tests_sha256": spec["tests_sha256"]})
    # Recheck current bytes, permissions and canonical receipts after execution.
    current_source, current = custody(root, company, run, trial, read_file)
    if current_source != source or current != provenance:
        raise ValueError("candidate custody changed during independent evaluation")
    return {"kind": "rust_acceptance", "passed": result["passed"], "native_result": result,
            "custody": provenance, "runtime": spec["runtime"], "tests_sha256": spec["tests_sha256"],
            "scope": "Exact submitted pure Rust bytes against the frozen owner suite; not general correctness, native run acceptance or source promotion",
            "promotion_authorized": False}
