"""Offline, bounded evidence preparation for the fictional mobilization POC.

This is an operator tool, not a general agent shell or arbitrary document importer.
Media extraction is fallible and must be reviewed before decisions are produced.
"""

import csv
import datetime as dt
import hashlib
import html
import json
import re
import shutil
import subprocess
import uuid
from pathlib import Path

PROFILE = "contractor-mobilization-v1"
COMPANY = "we-need-gents-lc"
PACK = "we-need-getns-llc"
TASK_FILES = (
    "onboarding-sop.txt",
    "employee-records.csv",
    "mobilization-requests.csv",
    "authorized-policy-update.wav",
    "onboarding-screen-demo.mp4",
)
COMPANY_FILES = (
    "00-company-handbook.txt",
    "departments/HR-manual.txt",
    "departments/Finance-manual.txt",
    "departments/IT-manual.txt",
    "departments/Supply-Chain-manual.txt",
    "registers/hr-clearance.csv",
    "registers/finance-cost-centers.csv",
    "registers/it-identity.csv",
    "registers/suppliers.csv",
)
# Reviewed source versions supported by this deliberately company-specific compiler.
# A policy edit requires source/rule review and a new profile, never just a reindex.
POLICY_SHA256 = {
    "task/onboarding-sop.txt": "45cf4724d19460bbc5e15750c009cbde4cdbd5d87bf5af4d9367ee0a1b364a1a",
    "company/00-company-handbook.txt": "d59449adaa9cf54dcd77d8db2419890ef18e48cd6dfe5b19f78cd133f6364767",
    "company/departments/HR-manual.txt": "5ac083c3bf84e4d1a1b889464d1bd25c056002719f77747bb6b1aaf8ed9c087e",
    "company/departments/Finance-manual.txt": "4185ef92991ace0819295cda2693d526a9160bdcfcc2493157a415fcb991e916",
    "company/departments/IT-manual.txt": "8f6173a4953fbc32253081a70f30129d8835a745ce0b677462fe67d1c4f85296",
    "company/departments/Supply-Chain-manual.txt": "49b17394cc13d549d68899e9e79aabbb10bd07a017180e65c1cb9119326ed288",
}


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    # Only used in a previously validated, newly created operator artifact directory.
    path.write_text(
        json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def confined(root, path):
    root, path = Path(root).absolute(), Path(path).absolute()
    if (
        not path.is_relative_to(root)
        or ".." in path.parts
        or "owner-evaluation" in path.parts
    ):
        raise ValueError("Evidence path outside the authorized root")
    cursor = root
    if cursor.is_symlink():
        raise ValueError("Evidence symlink rejected")
    for part in path.relative_to(root).parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError("Evidence symlink rejected")
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("Evidence path escaped its root")
    return path


def package_dir(root, company, package):
    if company != COMPANY:
        raise ValueError(
            "This POC profile is enabled only for we-need-gents-lc; other companies need their own reviewed rules"
        )
    package = str(uuid.UUID(package))
    return confined(root, root / "data/companies" / company / "evidence" / package)


def command(argv, timeout=180):
    result = subprocess.run(
        argv, capture_output=True, text=True, timeout=timeout, check=False
    )
    if result.returncode:
        raise ValueError(
            f"Local tool failed ({Path(argv[0]).name}): {result.stderr[-1200:]}"
        )
    return result.stdout


def prepare(root, company, package=None):
    root = Path(root).absolute()
    package = package or str(uuid.uuid4())
    destination = package_dir(root, company, package)
    if destination.exists():
        manifest = load(root, company, package)
        return package, manifest, True
    tools = {
        name: shutil.which(name) for name in ("ffmpeg", "ffprobe", "whisper", "swift")
    }
    if not all(tools.values()):
        raise ValueError(
            "Missing local tools: " + ", ".join(k for k, v in tools.items() if not v)
        )
    model = Path.home() / ".cache/whisper/tiny.pt"
    if not model.is_file():
        raise ValueError(
            "A cached Whisper tiny.pt is required; automatic model downloads are disabled"
        )
    source_root = confined(root, root / "docs/companies" / PACK)
    sources = []
    for folder, names in (("task", TASK_FILES), ("company", COMPANY_FILES)):
        for name in names:
            source = confined(source_root, source_root / folder / name)
            if not source.is_file() or source.stat().st_size > 100_000_000:
                raise ValueError(f"Missing or oversized source: {name}")
            sources.append((source, f"{folder}/{name}"))
    # Limit media duration before decoding so this is not an unbounded upload processor.
    video_duration = None
    for source, name in sources:
        if source.suffix in (".mp4", ".wav"):
            duration = float(
                command(
                    [
                        tools["ffprobe"],
                        "-v",
                        "error",
                        "-show_entries",
                        "format=duration",
                        "-of",
                        "csv=p=0",
                        str(source),
                    ]
                ).strip()
            )
            if not 0 < duration <= 300:
                raise ValueError("POC media must be between 0 and 300 seconds")
            if source.suffix == ".mp4":
                video_duration = duration
    destination.mkdir(parents=True, mode=0o700)
    print(
        "Copying allowlisted task/company evidence; originals remain unchanged.",
        flush=True,
    )
    originals = destination / "originals"
    originals.mkdir()
    for source, name in sources:
        target = originals / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    frames = destination / "frames"
    frames.mkdir()
    print(
        "Extracting timestamped video frames and local OCR observations...", flush=True
    )
    video = originals / "task/onboarding-screen-demo.mp4"
    # Select frames at exact integer timestamps, avoiding ambiguous fps-filter offsets.
    for second in range(int(video_duration) + 1):
        if second >= video_duration:
            break
        command(
            [
                tools["ffmpeg"],
                "-v",
                "error",
                "-nostdin",
                "-ss",
                str(second),
                "-i",
                str(video),
                "-frames:v",
                "1",
                "-vf",
                "scale=1280:-1",
                "-n",
                str(frames / f"frame-{second:03d}.png"),
            ]
        )
    frame_paths = sorted(frames.glob("*.png"))
    ocr_tool = root / "scripts/ocr_frames.swift"
    observations = json.loads(
        command([tools["swift"], str(ocr_tool), *map(str, frame_paths)], timeout=300)
    )
    for observation in observations:
        observation["timestamp_seconds"] = int(
            Path(observation["frame"]).stem.split("-")[-1]
        )
    write_json(destination / "ocr.json", observations)
    audio_inputs = [originals / "task/authorized-policy-update.wav", video]
    print(
        "Transcribing both audio streams with the cached local Whisper model...",
        flush=True,
    )
    command(
        [
            tools["whisper"],
            *map(str, audio_inputs),
            "--model",
            str(model),
            "--device",
            "cpu",
            "--language",
            "en",
            "--fp16",
            "False",
            "--output_format",
            "json",
            "--output_dir",
            str(destination),
        ],
        timeout=600,
    )
    corrections = {}
    for frame in observations:
        text = "\n".join(
            line["text"] for line in frame["lines"] if line["confidence"] >= 0.95
        )
        employee = re.search(r"Employee ID:\s*(E\d{3})\b", text)
        date = re.search(r"Training completed:\s*(\d{4}-\d{2}-\d{2})\b", text)
        if (
            employee
            and date
            and "Record status: VERIFIED" in text
            and "SOURCE SYSTEM" in text
        ):
            eid, value = employee[1], date[1]
            dt.date.fromisoformat(value)
            if (
                eid in corrections
                and corrections[eid]["training_completed_date"] != value
            ):
                raise ValueError(
                    "Conflicting visual corrections; manual investigation required"
                )
            corrections.setdefault(
                eid,
                {
                    "training_completed_date": value,
                    "frame": "frames/" + frame["frame"],
                    "timestamp_seconds": frame["timestamp_seconds"],
                },
            )
    candidate = {
        "reviewed": False,
        "reviewer": "",
        "effective_hire_date": "",
        "training_days": None,
        "manager_approval_required": None,
        "corrections": corrections,
        "warning": "Verify policy dates/requirements against the audio and every correction against its frame. OCR/transcripts are fallible.",
    }
    write_json(destination / "media-review.json", candidate)
    files = {
        str(p.relative_to(destination)): sha(p)
        for p in sorted(destination.rglob("*"))
        if p.is_file() and p.name != "media-review.json"
    }
    manifest = {
        "version": 1,
        "profile": PROFILE,
        "company_id": company,
        "package_id": package,
        "files": files,
        "whisper_model_sha256": sha(model),
        "ocr_tool_sha256": sha(ocr_tool),
        "tool_versions": {
            "ffmpeg": command([tools["ffmpeg"], "-version"]).splitlines()[0]
        },
        "media_review_sha256": None,
        "status": "review_required",
    }
    write_json(destination / "manifest.json", manifest)
    return package, manifest, False


def load(root, company, package, reviewed=False):
    directory = package_dir(root, company, package)
    manifest = json.loads(confined(directory, directory / "manifest.json").read_text())
    if (
        manifest.get("company_id") != company
        or manifest.get("package_id") != package
        or manifest.get("profile") != PROFILE
    ):
        raise ValueError("Evidence manifest company/package/profile mismatch")
    expected = {"originals/task/" + name for name in TASK_FILES} | {
        "originals/company/" + name for name in COMPANY_FILES
    }
    expected |= {
        "ocr.json",
        "authorized-policy-update.json",
        "onboarding-screen-demo.json",
    }
    if not expected.issubset(manifest["files"]):
        raise ValueError("Missing required evidence sources")
    for name, digest in manifest["files"].items():
        path = confined(directory, directory / name)
        if not path.is_file() or sha(path) != digest:
            raise ValueError(f"Evidence hash mismatch: {name}")
    if reviewed or manifest["status"] == "reviewed":
        review_path = confined(directory, directory / "media-review.json")
        if (
            manifest["status"] != "reviewed"
            or sha(review_path) != manifest["media_review_sha256"]
        ):
            raise ValueError(
                "Media must be explicitly reviewed before intake or execution"
            )
        review = json.loads(review_path.read_text())
        validate_review(review, directory)
    return manifest


def validate_review(review, directory):
    if review.get("reviewed") is not True or not review.get("reviewer", "").strip():
        raise ValueError("A named operator must review the media")
    dt.date.fromisoformat(review["effective_hire_date"])
    if (
        type(review["training_days"]) is not int
        or not 1 <= review["training_days"] <= 30
        or type(review["manager_approval_required"]) is not bool
    ):
        raise ValueError(
            "Review requires a bounded deadline and explicit manager approval rule"
        )
    observed = json.loads((directory / "ocr.json").read_text())
    for employee, correction in review["corrections"].items():
        dt.date.fromisoformat(correction["training_completed_date"])
        matches = [
            f
            for f in observed
            if "frames/" + f["frame"] == correction["frame"]
            and f["timestamp_seconds"] == correction["timestamp_seconds"]
        ]
        if len(matches) != 1:
            raise ValueError("Correction must reference an extracted timestamped frame")
        text = "\n".join(line["text"] for line in matches[0]["lines"])
        if not all(
            value in text
            for value in (employee, correction["training_completed_date"], "VERIFIED")
        ):
            raise ValueError(
                "Correction is not supported by OCR; inspect the frame manually"
            )


def review(root, company, package, reviewer, effective, days, approval):
    load(root, company, package)
    directory = package_dir(root, company, package)
    result = json.loads((directory / "media-review.json").read_text())
    if (directory / "manifest.json").is_symlink() or (
        directory / "media-review.json"
    ).is_symlink():
        raise ValueError("Review symlinks rejected")
    result.update(
        reviewed=True,
        reviewer=reviewer,
        effective_hire_date=effective,
        training_days=days,
        manager_approval_required=approval,
    )
    validate_review(result, directory)
    write_json(directory / "media-review.json", result)
    manifest = load(root, company, package)
    manifest.update(
        status="reviewed", media_review_sha256=sha(directory / "media-review.json")
    )
    write_json(directory / "manifest.json", manifest)


def read_csv(path, key):
    with path.open(newline="", encoding="utf-8-sig") as stream:
        rows = list(csv.DictReader(stream))
    keys = [row.get(key, "") for row in rows]
    if not all(keys) or len(keys) != len(set(keys)):
        raise ValueError(f"Missing/duplicate {key} in {path.name}")
    return {row[key]: row for row in rows}


def gate(*conditions):
    # Confirmed failure dominates missing evidence.
    return (
        "HOLD" if False in conditions else "UNKNOWN" if None in conditions else "PASS"
    )


def equals(row, field, value):
    return row[field] == value if row.get(field, "") else None


def decide(
    employee, request, hr, finance, identity, supplier, policy, correction, snapshot
):
    hire = dt.date.fromisoformat(employee["hire_date"])
    v2 = hire >= dt.date.fromisoformat(policy["effective_hire_date"])
    deadline = hire + dt.timedelta(days=policy["training_days"] if v2 else 10)
    completed = correction.get(
        "training_completed_date", employee.get("training_completed_date", "")
    )
    training = (
        "UNKNOWN"
        if not completed
        else "PASS"
        if dt.date.fromisoformat(completed) <= deadline
        else "LATE"
    )
    approval_required = v2 and policy["manager_approval_required"]
    if training == "LATE":
        access, reason = "HOLD", "TRAINING_LATE"
    elif approval_required and employee.get("manager_approved") == "no":
        access, reason = "HOLD", "APPROVAL_MISSING"
    elif training == "UNKNOWN" or (
        approval_required and employee.get("manager_approved") != "yes"
    ):
        access, reason = "UNKNOWN", "EVIDENCE_MISSING"
    else:
        access, reason = "ALLOW", "REQUIREMENTS_MET"
    amount = request.get("requested_cents", "")
    if amount and not re.fullmatch(r"\d+", amount):
        raise ValueError("requested_cents must be a nonnegative integer")
    coverage = supplier.get("insurance_valid_through", "")
    gates = {
        "hr_gate": gate(
            equals(hr, "clearance_code", "HRC-2"),
            equals(hr, "assignment_state", "ACTIVE"),
        ),
        "finance_gate": gate(
            equals(finance, "status", "ACTIVE"),
            equals(request, "charge_type", "MOB-IND"),
            int(amount) <= 120000 if amount else None,
        ),
        "it_gate": gate(
            equals(request, "role_code", "FIELD-READ"),
            equals(identity, "mfa_state", "VERIFIED"),
        ),
        "supply_chain_gate": gate(
            equals(supplier, "status", "APPROVED"),
            dt.date.fromisoformat(coverage) >= snapshot if coverage else None,
        ),
    }
    conditions = [access, *gates.values()]
    release = (
        "HOLD"
        if "HOLD" in conditions
        else "UNKNOWN"
        if "UNKNOWN" in conditions
        else "ALLOW"
    )
    eid = employee["employee_id"]
    return {
        "employee_id": eid,
        "policy_version": "V2" if v2 else "V1",
        "deadline": deadline.isoformat(),
        "effective_completion_date": completed,
        "training_decision": training,
        "task_access_decision": access,
        "reason_code": reason,
        **gates,
        "release_decision": release,
        "release_ticket": f"WNG|{eid}|{request['cost_center']}|READY"
        if release == "ALLOW"
        else "",
    }


def rule_sources(directory):
    # A deliberately bounded company-specific rule compiler. Do not silently apply
    # these rules to changed policies or to a different tenant.
    required = {
        "task/onboarding-sop.txt": [
            "2026-10-15",
            "hire_date plus 10 calendar days",
            "Hire date is day zero",
        ],
        "company/00-company-handbook.txt": [
            "all four department gates PASS",
            "Any confirmed HOLD",
            "WNG|<employee_id>|<cost_center>|READY",
        ],
        "company/departments/HR-manual.txt": [
            "clearance_code exactly HRC-2",
            "assignment_state exactly ACTIVE",
        ],
        "company/departments/Finance-manual.txt": [
            "charge_type exactly MOB-IND",
            "no greater than 120000",
        ],
        "company/departments/IT-manual.txt": [
            "role_code exactly FIELD-READ",
            "mfa_state exactly VERIFIED",
        ],
        "company/departments/Supply-Chain-manual.txt": [
            "supplier status APPROVED",
            "on or after the audit snapshot date",
        ],
    }
    for name, snippets in required.items():
        path = directory / "originals" / name
        if sha(path) != POLICY_SHA256[name]:
            raise ValueError(
                f"Unsupported policy source hash; reviewed rule-profile update required: {name}"
            )
        text = path.read_text()
        if name.startswith("company/") and "APPROVED" not in text[:250]:
            raise ValueError(f"Unapproved company policy: {name}")
        if not all(snippet in text for snippet in snippets):
            raise ValueError(
                f"Unsupported policy version; rule review required: {name}"
            )


def execute(root, company, run, package, expected_hash):
    manifest = load(root, company, package, reviewed=True)
    directory = package_dir(root, company, package)
    if sha(directory / "manifest.json") != expected_hash:
        raise ValueError(
            "Evidence changed since the SOW was frozen; create a new intake"
        )
    rule_sources(directory)
    output = confined(
        root, root / "data/companies" / company / "runs" / str(uuid.UUID(run)) / "audit"
    )
    if output.exists():
        summary = json.loads(confined(output, output / "run-summary.json").read_text())
        if (
            summary["evidence_sha256"] != expected_hash
            or summary["company_id"] != company
            or summary["run_id"] != run
        ):
            raise ValueError("Existing audit belongs to different evidence or scope")
        verify(directory, output)
        return summary, output, True
    original = directory / "originals"
    employees = read_csv(original / "task/employee-records.csv", "employee_id")
    requests = read_csv(original / "task/mobilization-requests.csv", "employee_id")
    if set(requests) != set(employees) or len(employees) != 12:
        raise ValueError(
            "The POC requires matching sets of exactly 12 employee/request IDs"
        )
    if any(row.get("worker_type") != "CONTRACTOR" for row in requests.values()):
        raise ValueError("This rule profile applies only to contractor mobilization")
    registers = {
        name: read_csv(original / f"company/registers/{name}.csv", key)
        for name, key in (
            ("hr-clearance", "employee_id"),
            ("finance-cost-centers", "cost_center"),
            ("it-identity", "employee_id"),
            ("suppliers", "supplier_id"),
        )
    }
    policy = json.loads((directory / "media-review.json").read_text())
    unknown_corrections = set(policy["corrections"]) - set(employees)
    if unknown_corrections:
        raise ValueError("Visual correction references an unknown employee")
    evidence = []

    def cite(source, locator, fact):
        eid = f"EV{len(evidence) + 1:03d}"
        evidence.append(
            {
                "evidence_id": eid,
                "source_file": source,
                "locator": locator,
                "observed_fact": fact,
                "source_sha256": manifest["files"].get(
                    "originals/" + source, manifest["media_review_sha256"]
                ),
            }
        )
        return eid

    policy_ids = [
        cite(
            "task/onboarding-sop.txt",
            "S01-S12",
            (original / "task/onboarding-sop.txt").read_text(),
        )
    ]
    for source, section in (
        ("00-company-handbook.txt", "HBK-03/HBK-04"),
        ("departments/HR-manual.txt", "HRC-07"),
        ("departments/Finance-manual.txt", "FIN-06"),
        ("departments/IT-manual.txt", "ITS-06"),
        ("departments/Supply-Chain-manual.txt", "SUP-02"),
    ):
        body = (original / "company" / source).read_text()
        selected = "\n".join(
            line
            for line in body.splitlines()
            if "For contractor mobilization" in line
            or "For the contractor mobilization" in line
            or "When final release" in line
        )
        policy_ids.append(cite("company/" + source, section, selected))
    for source in ("authorized-policy-update", "onboarding-screen-demo"):
        transcript = json.loads((directory / (source + ".json")).read_text())
        for segment in transcript["segments"]:
            policy_ids.append(
                cite(
                    "task/"
                    + source
                    + (".wav" if source == "authorized-policy-update" else ".mp4"),
                    f"audio {segment['start']:.2f}-{segment['end']:.2f}s",
                    segment["text"],
                )
            )
    policy_ids.append(
        cite("media-review.json", "operator-reviewed policy interpretation", policy)
    )
    decisions, findings, gate_facts = [], [], {}
    snapshot = dt.date(2026, 10, 15)
    for eid, employee in employees.items():
        request = requests[eid]
        hr = registers["hr-clearance"].get(eid, {})
        finance = registers["finance-cost-centers"].get(request.get("cost_center"), {})
        identity = registers["it-identity"].get(eid, {})
        supplier = registers["suppliers"].get(request.get("supplier_id"), {})
        correction = policy["corrections"].get(eid, {})
        gate_facts[eid] = {
            "hr_gate": {
                "clearance_code": hr.get("clearance_code"),
                "assignment_state": hr.get("assignment_state"),
                "requires": "HRC-2 and ACTIVE",
            },
            "finance_gate": {
                "cost_center": request.get("cost_center"),
                "status": finance.get("status"),
                "charge_type": request.get("charge_type"),
                "requested_cents": request.get("requested_cents"),
                "requires": "ACTIVE, MOB-IND, individual amount <= 120000 cents",
            },
            "it_gate": {
                "role_code": request.get("role_code"),
                "mfa_state": identity.get("mfa_state"),
                "requires": "FIELD-READ and VERIFIED",
            },
            "supply_chain_gate": {
                "supplier_id": request.get("supplier_id"),
                "status": supplier.get("status"),
                "insurance_valid_through": supplier.get("insurance_valid_through"),
                "requires": "APPROVED and coverage through 2026-10-15",
            },
        }
        citations = list(policy_ids)
        for source, locator, facts in (
            ("task/employee-records.csv", eid, employee),
            ("task/mobilization-requests.csv", eid, request),
            ("company/registers/hr-clearance.csv", eid, hr),
            (
                "company/registers/finance-cost-centers.csv",
                request.get("cost_center"),
                finance,
            ),
            ("company/registers/it-identity.csv", eid, identity),
            ("company/registers/suppliers.csv", request.get("supplier_id"), supplier),
        ):
            citations.append(cite(source, f"key={locator}", facts or {"missing": True}))
        if correction:
            citations.append(
                cite(
                    "task/onboarding-screen-demo.mp4",
                    f"video {correction['timestamp_seconds']}s; {correction['frame']}",
                    correction,
                )
            )
        decision = decide(
            employee,
            request,
            hr,
            finance,
            identity,
            supplier,
            policy,
            correction,
            snapshot,
        )
        decision["evidence_ids"] = citations
        decisions.append(decision)
        incorrect = [
            field
            for field, key in (
                ("recorded_training", "training_decision"),
                ("recorded_access", "task_access_decision"),
            )
            if employee[field] != decision[key]
        ]
        if incorrect:
            findings.append(
                {
                    "employee_id": eid,
                    "incorrect_fields": incorrect,
                    "recorded_values": {f: employee[f] for f in incorrect},
                    "corrected_values": {
                        f: decision[
                            "training_decision"
                            if f == "recorded_training"
                            else "task_access_decision"
                        ]
                        for f in incorrect
                    },
                    "explanation": f"{decision['policy_version']} deadline {decision['deadline']}; verified completion {decision['effective_completion_date']}; {decision['reason_code']}",
                    "evidence_ids": citations,
                }
            )
    output.mkdir(parents=True, mode=0o700)
    with (output / "decisions.csv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(decisions[0]))
        writer.writeheader()
        writer.writerows(
            {**row, "evidence_ids": json.dumps(row["evidence_ids"])}
            for row in decisions
        )
    write_json(output / "findings.json", findings)
    write_json(output / "evidence.json", evidence)
    rows = "".join(
        "<tr>"
        + "".join(
            f"<td>{html.escape(str(row[k]))}</td>"
            for k in (
                "employee_id",
                "training_decision",
                "task_access_decision",
                "hr_gate",
                "finance_gate",
                "it_gate",
                "supply_chain_gate",
                "release_decision",
                "release_ticket",
            )
        )
        + "</tr>"
        for row in decisions
    )
    blockers = []
    for row in decisions:
        for key, owner in (
            ("hr_gate", "HR"),
            ("finance_gate", "Finance"),
            ("it_gate", "IT"),
            ("supply_chain_gate", "Supply Chain"),
        ):
            if row[key] != "PASS":
                blockers.append(
                    f"{row['employee_id']}: {key}={row[key]}. Facts: {json.dumps(gate_facts[row['employee_id']][key])}. Owner: {owner}. Reconcile the cited request/register with the controlling policy; obtain corrected evidence before release."
                )
        if row["task_access_decision"] != "ALLOW":
            blockers.append(
                f"{row['employee_id']}: {row['reason_code']}. Owner: HR/manager. Review training deadline, verified completion and required approval; do not provision access."
            )
    (output / "report.html").write_text(
        "<!doctype html><meta charset='utf-8'><title>Fictional mobilization audit</title><h1>We Need Gents LC — 2026-10-15</h1><p>Recommendation only. No real accounts, payments or release actions executed.</p><table border='1'><tr><th>Employee</th><th>Training</th><th>Access</th><th>HR</th><th>Finance</th><th>IT</th><th>Supplier</th><th>Release</th><th>Ticket</th></tr>"
        + rows
        + "</table><h2>Blockers, owners and next actions</h2><ul>"
        + "".join("<li>" + html.escape(b) + "</li>" for b in blockers)
        + "</ul><p>See evidence.json for exact source facts and locators. See run-summary.json for limitations.</p>"
    )
    summary = {
        "company_id": company,
        "run_id": run,
        "profile": PROFILE,
        "evidence_package": package,
        "evidence_sha256": expected_hash,
        "snapshot": snapshot.isoformat(),
        "status": "artifacts_generated",
        "employee_count": len(decisions),
        "finding_count": len(findings),
        "actors": [{"name": "operator deterministic mobilization tool", "model": None}],
        "model_calls": 0,
        "tokens": 0,
        "model_calls_scope": "deterministic audit execution only; ASR preprocessing and Gent reviews are separate",
        "preprocessing": {
            "asr_model": "Whisper tiny",
            "asr_inputs_transcribed": 2,
            "asr_token_usage": None,
            "model_sha256": manifest.get("whisper_model_sha256"),
            "device": "cpu",
            "frame_count": sum(
                name.startswith("frames/") for name in manifest["files"]
            ),
        },
        "external_spend_usd": 0,
        "cache_hit": False,
        "blockers": blockers,
        "decision_table": decisions,
        "limitations": [
            "Company-specific rule compiler, not general software execution or legal compliance certification.",
            "Media interpretation requires named operator review; no independent owner ground truth consulted.",
            "LLM role reviews are separate and their calls/tokens are in the harness events.",
            "No Gent sleep/wake inference cycle is proven by this tool. No production actions authorized.",
        ],
        "artifact_sha256": {
            name: sha(output / name)
            for name in (
                "decisions.csv",
                "findings.json",
                "evidence.json",
                "report.html",
            )
        },
    }
    write_json(output / "run-summary.json", summary)
    checks = verify(directory, output)
    summary.update(status="verified", checks=checks)
    write_json(output / "run-summary.json", summary)
    return summary, output, False


def verify(directory, output):
    """Separate artifact verifier: recompute dates and gates from source records.

    Does not consult owner evaluation. Independent control-flow checks catch
    coverage/join/schema/ticket/citation and artifact mutation, not model accuracy.
    """
    summary = json.loads((output / "run-summary.json").read_text())
    for name, digest in summary["artifact_sha256"].items():
        if sha(confined(output, output / name)) != digest:
            raise ValueError("Artifact hash mismatch: " + name)
    actual = read_csv(output / "decisions.csv", "employee_id")
    source_rows = read_csv(
        directory / "originals/task/employee-records.csv", "employee_id"
    )
    requests = read_csv(
        directory / "originals/task/mobilization-requests.csv", "employee_id"
    )
    policy = json.loads((directory / "media-review.json").read_text())
    evidence = json.loads((output / "evidence.json").read_text())
    by_id = {item["evidence_id"]: item for item in evidence}
    manifest = json.loads((directory / "manifest.json").read_text())
    for item in evidence:
        source, locator, fact = (
            item["source_file"],
            item["locator"],
            item["observed_fact"],
        )
        if source == "media-review.json":
            expected_hash = manifest["media_review_sha256"]
            if fact != policy:
                raise ValueError("Evidence policy interpretation changed")
        else:
            expected_hash = manifest["files"].get("originals/" + source)
            if not expected_hash:
                raise ValueError("Evidence source is not in the frozen manifest")
            original = confined(directory, directory / "originals" / source)
            if sha(original) != expected_hash:
                raise ValueError("Evidence source content changed")
            if source.endswith(".csv"):
                if not locator.startswith("key="):
                    raise ValueError("CSV evidence needs a stable join key")
                with original.open(newline="", encoding="utf-8-sig") as stream:
                    all_rows = list(csv.DictReader(stream))
                matches = [r for r in all_rows if locator[4:] in r.values()]
                if fact not in matches and not (
                    fact == {"missing": True} and not matches
                ):
                    raise ValueError("Evidence CSV fact does not match the source")
            elif locator.startswith("audio "):
                transcript = json.loads(
                    (directory / (Path(source).stem + ".json")).read_text()
                )
                if not any(
                    locator == f"audio {s['start']:.2f}-{s['end']:.2f}s"
                    and fact == s["text"]
                    for s in transcript["segments"]
                ):
                    raise ValueError("Evidence transcript/time locator mismatch")
            elif locator.startswith("video "):
                if (
                    fact not in policy["corrections"].values()
                    or locator != f"video {fact['timestamp_seconds']}s; {fact['frame']}"
                ):
                    raise ValueError("Evidence video correction/time locator mismatch")
            elif isinstance(fact, str):
                if not all(line in original.read_text() for line in fact.splitlines()):
                    raise ValueError("Evidence policy quote does not match source")
            else:
                raise ValueError("Unsupported evidence schema")
        if item["source_sha256"] != expected_hash:
            raise ValueError("Evidence source hash mismatch")
    if (
        len(by_id) != len(evidence)
        or set(actual) != set(source_rows)
        or set(requests) != set(source_rows)
        or len(actual) != 12
    ):
        raise ValueError("Coverage, joins or evidence identifiers failed")
    expected_findings = set()
    for eid, row in actual.items():
        ids = json.loads(row["evidence_ids"])
        if (
            not ids
            or not set(ids).issubset(by_id)
            or not any(by_id[i]["locator"] == f"key={eid}" for i in ids)
        ):
            raise ValueError("Missing employee-specific evidence")
        employee = source_rows[eid]
        is_v2 = employee["hire_date"] >= policy["effective_hire_date"]
        deadline = dt.date.fromisoformat(employee["hire_date"]) + dt.timedelta(
            days=policy["training_days"] if is_v2 else 10
        )
        completed = (
            policy["corrections"]
            .get(eid, {})
            .get("training_completed_date", employee["training_completed_date"])
        )
        expected_training = (
            "UNKNOWN"
            if not completed
            else ("LATE" if completed > deadline.isoformat() else "PASS")
        )
        if (
            row["deadline"] != deadline.isoformat()
            or row["effective_completion_date"] != completed
            or row["training_decision"] != expected_training
            or row["policy_version"] != ("V2" if is_v2 else "V1")
        ):
            raise ValueError("Date/policy/source precedence verification failed")
        needed = is_v2 and policy["manager_approval_required"]
        if expected_training == "LATE":
            access, reason = "HOLD", "TRAINING_LATE"
        elif needed and employee["manager_approved"] == "no":
            access, reason = "HOLD", "APPROVAL_MISSING"
        elif expected_training == "UNKNOWN" or (
            needed and employee["manager_approved"] != "yes"
        ):
            access, reason = "UNKNOWN", "EVIDENCE_MISSING"
        else:
            access, reason = "ALLOW", "REQUIREMENTS_MET"
        if row["task_access_decision"] != access or row["reason_code"] != reason:
            raise ValueError("Access verification failed")
        request = requests[eid]
        gates = {}
        for key, filename, join_key, lookup, tests in (
            (
                "hr_gate",
                "hr-clearance",
                "employee_id",
                eid,
                [("clearance_code", "HRC-2"), ("assignment_state", "ACTIVE")],
            ),
            (
                "finance_gate",
                "finance-cost-centers",
                "cost_center",
                request.get("cost_center"),
                [("status", "ACTIVE")],
            ),
            ("it_gate", "it-identity", "employee_id", eid, [("mfa_state", "VERIFIED")]),
            (
                "supply_chain_gate",
                "suppliers",
                "supplier_id",
                request.get("supplier_id"),
                [("status", "APPROVED")],
            ),
        ):
            record = read_csv(
                directory / f"originals/company/registers/{filename}.csv", join_key
            ).get(lookup, {})
            values = [
                None if not record.get(field) else record[field] == expected
                for field, expected in tests
            ]
            if key == "finance_gate":
                values += [
                    equals(request, "charge_type", "MOB-IND"),
                    int(request["requested_cents"]) <= 120000
                    if request.get("requested_cents")
                    else None,
                ]
            if key == "it_gate":
                values += [equals(request, "role_code", "FIELD-READ")]
            if key == "supply_chain_gate":
                values += [
                    record["insurance_valid_through"] >= summary["snapshot"]
                    if record.get("insurance_valid_through")
                    else None
                ]
            gates[key] = (
                "HOLD"
                if any(v is False for v in values)
                else "UNKNOWN"
                if any(v is None for v in values)
                else "PASS"
            )
            if row[key] != gates[key]:
                raise ValueError("Department gate verification failed")
        outcomes = [access, *gates.values()]
        release = (
            "HOLD"
            if "HOLD" in outcomes
            else "ALLOW"
            if access == "ALLOW" and all(v == "PASS" for v in gates.values())
            else "UNKNOWN"
        )
        ticket = (
            f"WNG|{eid}|{request['cost_center']}|READY" if release == "ALLOW" else ""
        )
        if row["release_decision"] != release or row["release_ticket"] != ticket:
            raise ValueError("Release gate/ticket verification failed")
        if (
            employee["recorded_training"] != expected_training
            or employee["recorded_access"] != access
        ):
            expected_findings.add(eid)
    findings = json.loads((output / "findings.json").read_text())
    if (
        len(findings) != len(expected_findings)
        or {f["employee_id"] for f in findings} != expected_findings
    ):
        raise ValueError("Discrepancy coverage verification failed")
    for finding in findings:
        eid = finding["employee_id"]
        expected_fields = [
            field
            for field, key in (
                ("recorded_training", "training_decision"),
                ("recorded_access", "task_access_decision"),
            )
            if source_rows[eid][field] != actual[eid][key]
        ]
        expected_corrected = {
            field: actual[eid][
                "training_decision"
                if field == "recorded_training"
                else "task_access_decision"
            ]
            for field in expected_fields
        }
        if (
            finding["incorrect_fields"] != expected_fields
            or finding["corrected_values"] != expected_corrected
            or finding["recorded_values"]
            != {f: source_rows[eid][f] for f in expected_fields}
            or not set(finding["evidence_ids"]).issubset(by_id)
        ):
            raise ValueError("Discrepancy fields/values/evidence verification failed")
    return {
        name: True
        for name in (
            "coverage",
            "joins",
            "dates_and_precedence",
            "four_gates",
            "release_logic",
            "ticket_format",
            "evidence_references",
            "artifact_hashes",
            "discrepancy_coverage",
        )
    }
