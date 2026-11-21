#!/usr/bin/env python3
"""Local operator CLI. Model outputs never execute this CLI or authorize its actions."""

import argparse
import datetime as dt
import json
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMPANY = "we-need-gents-lc"
MAX_TASK_BYTES = 10000  # Match the company intake API's field limit.


def api(args, path, body=None):
    url = urllib.parse.urlparse(args.server)
    if url.scheme != "http" or url.hostname not in ("127.0.0.1", "localhost", "::1"):
        raise ValueError("This operator CLI requires a loopback HTTP harness")
    req = urllib.request.Request(
        args.server.rstrip("/") + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


def company(args, op, **data):
    if not args.company or any(
        c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in args.company
    ):
        raise ValueError("invalid company ID")
    return api(args, f"/api/companies/{args.company}", {"op": op, **data})


def safe_path(args, *parts):
    target = ROOT / "data" / "companies" / args.company
    for part in parts:
        if Path(part).name != part or part in (".", ".."):
            raise ValueError("unsafe artifact component")
        target /= part
    cursor = ROOT
    for part in target.relative_to(ROOT).parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError("artifact symlinks are forbidden")
    return target


def doctor():
    organization = json.loads((ROOT / "config/organization.json").read_text())
    models = list((ROOT / "models").rglob("*.gguf"))
    tensors = list((ROOT / "models").rglob("*.safetensors"))
    print("Local model files:")
    for path in models + tensors:
        print(f"  {path.stat().st_size / 1e9:5.2f} GB  {path.relative_to(ROOT)}")
    for executable in ["llama-server", "cargo", "node", "soffice"]:
        print(f"{executable}: {shutil.which(executable) or 'MISSING'}")
    python = ROOT / ".venv-inference/bin/python"
    print(
        f"MLX environment: {'installed' if python.exists() else 'run sh scripts/setup-local.sh'}"
    )
    missing = []
    config = json.loads((ROOT / "config/company.local.json").read_text())
    for agent in config["agents"]:
        for slot in ["planner", "worker"]:
            path = ROOT / agent[slot]["path"]
            if not path.is_file() or path.stat().st_size == 0:
                missing.append(str(path))
    print(
        f"Organization: {len(organization['roles'])} role profiles, one loaded model at a time"
    )
    print(
        "Company workflow: admin-reviewed web, isolated code/tests, email, model operations and local training."
    )
    print("Paid inference: denied by default. No cloud API key required.")
    if missing:
        raise ValueError("Missing model files: " + ", ".join(sorted(set(missing))))


def onboard(args):
    source = (ROOT / args.docs).absolute()
    allowed = (ROOT / "docs/companies").resolve()
    if source.is_symlink() or not source.resolve().is_relative_to(allowed):
        raise ValueError("onboarding sources must be under docs/companies")
    # Select exactly company/, never the adjacent owner-evaluation directory.
    source = source / "company" if (source / "company").is_dir() else source
    if source.name != "company":
        raise ValueError("select the company/ document directory or its parent")
    company(args, "register", name=args.name)
    count = 0
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in (
            ".md",
            ".txt",
            ".csv",
            ".json",
        ):
            continue
        if path.is_symlink() or not path.resolve().is_relative_to(source.resolve()):
            raise ValueError("onboarding symlink rejected")
        if path.stat().st_size > 60000:
            raise ValueError(
                f"Split document into reviewed sections under 60KB: {path}"
            )
        text = path.read_text()
        # Keep each HTTP ingestion under the harness's body limit.
        for index in range(0, len(text), 3000):
            company(
                args,
                "ingest",
                source=f"{path.relative_to(source)}#chunk-{index // 3000}",
                text=text[index : index + 3000],
            )
        count += 1
    print(
        f"Onboarded {count} company documents for {args.company}. Owner evaluation excluded."
    )


def answer(label, default=None):
    while True:
        result = (
            input(f"{label}" + (f" [{default}]" if default else "") + ": ").strip()
            or default
        )
        if result:
            return result


def task_answer(args):
    task_file = getattr(args, "task_file", None)
    if task_file is None:
        value = answer("What should the Gents deliver? (text or @path/to/task.txt)")
        if not value.startswith("@"):
            if len(value.encode("utf-8")) > MAX_TASK_BYTES:
                raise ValueError(
                    f"Task exceeds {MAX_TASK_BYTES} UTF-8 bytes; shorten it before intake"
                )
            return value
        task_file = value[1:].strip()
        if not task_file:
            raise ValueError("Supply a task file path after @")
    path = Path(task_file).expanduser()
    if "owner-evaluation" in path.parts or "owner-evaluation" in path.resolve().parts:
        raise ValueError("Owner evaluation cannot be used as a task prompt")
    if not path.is_file():
        raise ValueError(f"Task file not found: {path}")
    with path.open("rb") as stream:
        content = stream.read(MAX_TASK_BYTES + 4)  # Allow an optional UTF-8 BOM.
    try:
        task = content.decode("utf-8-sig").strip()
    except UnicodeDecodeError as error:
        raise ValueError("Task file must be UTF-8 text") from error
    if not task or "\x00" in task:
        raise ValueError("Task file must contain nonempty text without NUL characters")
    if len(content) > MAX_TASK_BYTES + 3 or len(task.encode("utf-8")) > MAX_TASK_BYTES:
        raise ValueError(
            f"Task exceeds {MAX_TASK_BYTES} UTF-8 bytes; shorten it before intake"
        )
    print(
        f"Loaded task from {path} ({len(task.encode('utf-8'))} UTF-8 bytes). Supporting files are not attached."
    )
    return task


def intake(args):
    idea = task_answer(args)
    evidence = None
    package = getattr(args, "evidence_package", None)
    if package:
        import task_evidence

        task_evidence.load(ROOT, args.company, package, reviewed=True)
        evidence = {
            "profile": task_evidence.PROFILE,
            "package_id": package,
            "sha256": task_evidence.sha(
                task_evidence.package_dir(ROOT, args.company, package) / "manifest.json"
            ),
        }
    kind = answer("Deliverable type: text or software", "text")
    if kind not in ("text", "software"):
        raise ValueError("deliverable type must be text or software")
    print(
        "Enter measurable acceptance criteria, one per line. Blank line ends the list."
    )
    criteria = []
    while len(criteria) < 8:
        item = input(f"Criterion {len(criteria) + 1}: ").strip()
        if not item:
            if criteria:
                break
            continue
        criteria.append(item)
    data = {
        "idea": idea,
        "acceptance": "\n".join(criteria),
        "deliverable_type": kind,
        "exclusions": answer(
            "What is out of scope?", "Paid services, deployment, real signatures"
        ),
        "timeline": answer(
            "Desired timeline and milestones (estimates only)",
            "No deadline. Plan, deliver, review",
        ),
        "company_signer": answer(
            "Authorized company signatory name (signature pending)"
        ),
        "provider_signer": answer(
            "Authorized Gents signatory name (signature pending)"
        ),
        "questions": answer(
            "Open questions, missing data or constraints", "None supplied"
        ),
        "budget_usd": 0,
        "signature_status": "unsigned",
    }
    if evidence:
        data["task_evidence"] = evidence
    deadline = input(
        "Optional deadline with timezone, e.g. 2026-10-20T17:00:00-05:00 (blank = unknown): "
    ).strip()
    if deadline:
        parsed = dt.datetime.fromisoformat(deadline)
        if parsed.tzinfo is None:
            raise ValueError("deadline must include a timezone offset")
        data["deadline_ms"] = int(parsed.timestamp() * 1000)
    run = str(uuid.uuid4())
    state = company(args, "intake", run_id=run, intake=data)
    job = next(j for j in state["jobs"] if j["run_id"] == run)
    directory = safe_path(args, "runs", run)
    directory.mkdir(parents=True, exist_ok=True)
    sow = directory / "SOW.md"
    sow.write_text(
        f"# Scope of Work draft\n\nCompany: {state['name']} ({args.company})\nRun: {run}\n\n"
        + "\n\n".join(
            f"## {key.replace('_', ' ').title()}\n\n{value}"
            for key, value in data.items()
        )
        + f"\n\nSHA-256 of frozen intake: {job['digest']}\n\nBoth signatures are pending. A local operator approval authorizes a test only.\n"
    )
    print(f"Review {sow}\nRun ID: {run}\nDigest: {job['digest']}")
    print(
        f"After review: python3 scripts/gents.py --company {args.company} approve {run} {job['digest']}"
    )


def job_for(args):
    state = company(args, "status")
    return next(j for j in state["jobs"] if j["run_id"] == args.run)


def tail(args):
    seen = 0
    while True:
        state = api(args, "/api/runs/" + args.run)
        if state.get("company_id") != args.company:
            raise ValueError("run belongs to a different company")
        for event in state["events"][seen:]:
            timestamp = dt.datetime.fromtimestamp(
                event["timestamp_ms"] / 1000
            ).isoformat(timespec="seconds")
            # JSON encoding neutralizes terminal escape sequences from model/company text.
            print(
                json.dumps(
                    {
                        "time": timestamp,
                        "company_id": args.company,
                        "run_id": args.run,
                        "gent": event["agent"],
                        "role": event["role"],
                        "model": event.get("model"),
                        "kind": event["kind"],
                        "summary": event["message"],
                        "reply": event.get("reply"),
                    },
                    ensure_ascii=True,
                ),
                flush=True,
            )
        seen = len(state["events"])
        if state["status"] != "running":
            print(f"Run state: {state['status']}")
            return
        time.sleep(1)


def report(args, quarter=None):
    state = company(args, "status")
    now = dt.datetime.now(ZoneInfo("America/Chicago"))
    quarter = quarter or f"{now:%Y-%m-%d}-Q{now.hour // 6 + 1}"
    folder = safe_path(args, "reports")
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"{quarter}.json"
    payload = {
        "quarter": quarter,
        "timezone": "America/Chicago",
        "company": state,
        "generated_at": now.isoformat(),
        "report_kind": "Actual checkpoints and declared milestones. No invented progress.",
    }
    path.write_text(json.dumps(payload, indent=2))
    subprocess.run(
        [sys.executable, str(ROOT / "scripts/quarter_report.py"), str(path)], check=True
    )
    return path


def daemon(args):
    print(
        "Scheduler running. America/Chicago, 00:00/06:00/12:00/18:00. Dream housekeeping during 02:00-04:00."
    )
    while True:
        now = dt.datetime.now(ZoneInfo("America/Chicago"))
        quarter = f"{now:%Y-%m-%d}-Q{now.hour // 6 + 1}"
        try:
            directory = safe_path(args, "reports")
            if not (directory / f"{quarter}.pptx").exists():
                report(args, quarter)
            marker = safe_path(args, "dream", f"{now:%Y-%m-%d}.json")
            if 2 <= now.hour < 4 and not marker.exists():
                state = company(args, "dream")
                marker.parent.mkdir(parents=True, exist_ok=True)
                marker.write_text(json.dumps(state))
        except (OSError, ValueError, subprocess.CalledProcessError) as exc:
            print(
                f"Scheduler checkpoint failed, will retry: {exc}",
                file=sys.stderr,
                flush=True,
            )
        time.sleep(30)


def projection(args):
    state = api(args, "/api/runs/" + args.run)
    if state.get("company_id") != args.company:
        raise ValueError("company mismatch")
    events = state["events"]
    if not events:
        raise ValueError("no measurements yet")
    wall = (events[-1]["timestamp_ms"] - events[0]["timestamp_ms"]) / 1000
    calls = [e["reply"] for e in events if e.get("reply")]
    measured = sum(c["latency_ms"] for c in calls) / 1000
    inference = min(wall, measured)
    print(
        json.dumps(
            {
                "measured_wall_seconds": wall,
                "completed_model_calls": len(calls),
                "measured_request_seconds": measured,
                "excluded": "model loading, tooling, human review and idle time do not scale with decoding speed",
                "hypothetical_scenarios": [
                    {
                        "inference_speed_multiplier": s,
                        "estimated_seconds": round(wall - inference + inference / s, 2),
                    }
                    for s in (1.5, 2, 4)
                ],
                "interpretation": "Conditional estimate, not a benchmark or a GPU purchase recommendation. Measure the same workload on target hardware.",
            },
            indent=2,
        )
    )


def training_prepare(args, smoke=False):
    from train_role import digest, samples

    company(args, "status")
    run = str(uuid.uuid4())
    destination = safe_path(args, "training", "datasets", run)
    destination.mkdir(parents=True, exist_ok=False)
    if smoke:
        for split, labels in [
            ("train", ["alpha", "beta"]),
            ("valid", ["gamma"]),
            ("test", ["delta"]),
        ]:
            rows = [
                {
                    "company_id": args.company,
                    "role": args.role,
                    "reviewed": True,
                    "license": "CC0-1.0 synthetic smoke fixture",
                    "source": "synthetic-runtime-smoke",
                    "messages": [
                        {
                            "role": "user",
                            "content": f"For test {label}, reply with a JSON object whose status is ok.",
                        },
                        {"role": "assistant", "content": '{"status":"ok"}'},
                    ],
                }
                for label in labels
            ]
            (destination / f"{split}.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n"
            )
    else:
        source = Path(args.data).absolute()
        if (
            "owner-evaluation" in str(source)
            or "owner-evaluation" in str(source.resolve())
            or source.is_symlink()
        ):
            raise ValueError("owner evaluation and dataset symlinks are excluded")
        samples(source, args.company, args.role)
        for split in ("train", "valid", "test"):
            shutil.copyfile(source / f"{split}.jsonl", destination / f"{split}.jsonl")
    counts = {
        k: len(v)
        for k, v in samples(destination, args.company, args.role, smoke).items()
    }
    manifest = {
        "company_id": args.company,
        "training_id": run,
        "role": args.role,
        "base_model": args.base_model,
        "dataset_sha256": {
            split: digest(destination / f"{split}.jsonl")
            for split in ("train", "valid", "test")
        },
        "dataset_counts": counts,
        "iterations": 1 if smoke else args.iterations,
        "sequence_length": 512,
        "smoke": smoke,
        "reviewed_dataset": True,
    }
    company(args, "training_register", manifest=manifest)
    print(
        f"Training ID: {run}\nRole: {args.role}\nDataset: {counts}\nSmoke only: {smoke}"
    )
    if smoke:
        company(args, "training_start", training_id=run)
        print(
            "Synthetic one-step local training queued. This adapter cannot be promoted."
        )
    else:
        print(
            f"Start: python3 scripts/gents.py --company {args.company} training-start {run}"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", default="http://127.0.0.1:8788")
    parser.add_argument("--company", default=DEFAULT_COMPANY)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("doctor")
    onboard_parser = sub.add_parser("onboard")
    onboard_parser.add_argument("--name", default="We Need Gents LLC")
    onboard_parser.add_argument("--docs", default="docs/companies/we-need-getns-llc")
    intake_parser = sub.add_parser("intake")
    intake_parser.add_argument(
        "--task-file",
        help="Read the delivery answer from a UTF-8 text file (up to 10000 bytes)",
    )
    intake_parser.add_argument(
        "--evidence-package",
        help="Reviewed company evidence package ID, frozen into this SOW",
    )
    prepare_parser = sub.add_parser(
        "prepare-task",
        help="Prepare the local fictional-company task sources and media",
    )
    prepare_parser.add_argument(
        "--package",
        help="Validate and reuse an existing package without repeating preprocessing",
    )
    sub.add_parser(
        "review-task", help="Review media and record the policy facts before intake"
    ).add_argument("package")
    sub.add_parser(
        "verify-audit", help="Recheck an existing run's deterministic audit artifacts"
    ).add_argument("run")
    for command in ("start", "tail", "cancel", "finish", "projection"):
        sub.add_parser(command).add_argument("run")
    sub.add_parser("recall").add_argument("run")
    approval = sub.add_parser("approve")
    approval.add_argument("run")
    approval.add_argument("digest")
    for command in ("status", "dream", "archive", "report", "daemon"):
        sub.add_parser(command)
    for command in ("training-prepare", "training-smoke"):
        train = sub.add_parser(command)
        train.add_argument("--role", default="backend-engineer")
        train.add_argument("--base-model", default="models/gemma-4-E4B-it")
        train.add_argument("--iterations", type=int, default=100)
        if command == "training-prepare":
            train.add_argument(
                "--data",
                required=True,
                help="Directory with reviewed train/valid/test.jsonl",
            )
    for command in ("training-start", "training-cancel", "training-retire"):
        sub.add_parser(command).add_argument("training_id")
    promote = sub.add_parser("training-promote")
    promote.add_argument("training_id")
    promote.add_argument(
        "review", help="JSON evaluation attestations and exact adapter SHA-256"
    )
    meeting = sub.add_parser("all-hands")
    meeting.add_argument("run")
    meeting.add_argument("brief")
    milestone = sub.add_parser("milestone")
    milestone.add_argument("run")
    milestone.add_argument("--id", required=True)
    milestone.add_argument("--title", required=True)
    milestone.add_argument("--agent", default="PM Gent")
    milestone.add_argument(
        "--state",
        choices=["planned", "in_progress", "done", "blocked"],
        default="planned",
    )
    milestone.add_argument("--due", help="ISO timestamp with timezone")
    milestone.add_argument("--evidence", default="")
    capability = sub.add_parser("capability")
    capability.add_argument(
        "manifest", help="Reviewed local JSON manifest to quarantine"
    )
    args = parser.parse_args()
    if args.command == "doctor":
        doctor()
    elif args.command == "onboard":
        onboard(args)
    elif args.command == "intake":
        intake(args)
    elif args.command == "prepare-task":
        import task_evidence

        package, manifest, hit = task_evidence.prepare(ROOT, args.company, args.package)
        directory = task_evidence.package_dir(ROOT, args.company, package)
        print(
            f"Package: {package}\nDirectory: {directory}\nValidated cache hit: {hit}\nStatus: {manifest['status']}"
        )
        print(
            f"Next: python3 scripts/gents.py --company {args.company} review-task {package}"
        )
    elif args.command == "review-task":
        import task_evidence

        task_evidence.load(ROOT, args.company, args.package)
        directory = task_evidence.package_dir(ROOT, args.company, args.package)
        for name in ("authorized-policy-update", "onboarding-screen-demo"):
            transcript = json.loads((directory / f"{name}.json").read_text())
            print(f"\n{name}: machine transcript, not independently verified")
            for segment in transcript["segments"]:
                print(
                    json.dumps(
                        {
                            "start": segment["start"],
                            "end": segment["end"],
                            "text": segment["text"],
                        }
                    )
                )
        print("\nCandidate visual corrections (inspect the referenced PNGs):")
        print(
            json.dumps(
                json.loads((directory / "media-review.json").read_text())[
                    "corrections"
                ],
                indent=2,
            )
        )
        print(f"Original media and timestamped frames: {directory}")
        if (
            answer(
                "Have you checked both original audio streams and the correction frames? Type yes to attest",
                "no",
            )
            != "yes"
        ):
            raise ValueError("Media review pending; no approval recorded")
        reviewer = answer("Operator reviewer name (simulation label allowed)")
        effective = answer("Authorized notice: effective hire date (YYYY-MM-DD)")
        days = int(answer("Authorized notice: training deadline in calendar days"))
        approval = answer("Authorized notice: manager approval required? yes/no")
        if approval not in ("yes", "no"):
            raise ValueError("Enter yes or no for manager approval")
        task_evidence.review(
            ROOT,
            args.company,
            args.package,
            reviewer,
            effective,
            days,
            approval == "yes",
        )
        print("Reviewed media interpretation frozen. Next:")
        print(
            f"python3 scripts/gents.py --company {args.company} intake --task-file docs/companies/we-need-getns-llc/CLI-AUDIT-TASK.txt --evidence-package {args.package}"
        )
    elif args.command == "verify-audit":
        import task_evidence

        job = job_for(args)
        binding = job["intake"].get("task_evidence")
        if not binding:
            raise ValueError("Run has no frozen task evidence")
        task_evidence.load(ROOT, args.company, binding["package_id"], reviewed=True)
        directory = task_evidence.package_dir(ROOT, args.company, binding["package_id"])
        if task_evidence.sha(directory / "manifest.json") != binding["sha256"]:
            raise ValueError("Evidence no longer matches the SOW")
        print(
            json.dumps(
                task_evidence.verify(
                    directory, safe_path(args, "runs", args.run, "audit")
                ),
                indent=2,
            )
        )
    elif args.command in ("training-prepare", "training-smoke"):
        training_prepare(args, args.command == "training-smoke")
    elif args.command in ("training-start", "training-cancel", "training-retire"):
        state = company(
            args, args.command.replace("-", "_"), training_id=args.training_id
        )
        print(json.dumps(state["training"], indent=2))
    elif args.command == "training-promote":
        state = company(
            args,
            "training_promote",
            training_id=args.training_id,
            review=json.loads(Path(args.review).read_text()),
        )
        print(json.dumps(state["training"], indent=2))
    elif args.command == "all-hands":
        state = company(args, "all_hands", run_id=args.run, brief=args.brief)
        print(json.dumps(state["meetings"], indent=2))
    elif args.command == "milestone":
        due = None
        if args.due:
            parsed = dt.datetime.fromisoformat(args.due)
            if parsed.tzinfo is None:
                raise ValueError("due must include a timezone")
            due = int(parsed.timestamp() * 1000)
        state = company(
            args,
            "milestone",
            run_id=args.run,
            id=args.id,
            title=args.title,
            agent=args.agent,
            due_ms=due,
            state=args.state,
            evidence=args.evidence,
        )
        print(json.dumps(state["milestones"], indent=2))
    elif args.command == "approve":
        print(
            json.dumps(
                company(args, "approve", run_id=args.run, digest=args.digest), indent=2
            )
        )
    elif args.command == "start":
        job = job_for(args)
        binding = job["intake"].get("task_evidence")
        if binding:
            import task_evidence

            if api(args, "/api/config").get("task_evidence_version") != 1:
                raise ValueError(
                    "Restart the harness with sh scripts/run-company.sh to enable task evidence"
                )
            approval = json.loads(job["approval"] or "{}")
            if job["state"] != "approved" or approval.get("sha256") != job["digest"]:
                raise ValueError("Approve the frozen SOW before executing the audit")
            summary, output, hit = task_evidence.execute(
                ROOT, args.company, args.run, binding["package_id"], binding["sha256"]
            )
            directory = task_evidence.package_dir(
                ROOT, args.company, binding["package_id"]
            )
            # The server rejects unsupported operations before any model work starts.
            company(
                args,
                "attach_evidence",
                run_id=args.run,
                manifest_sha256=binding["sha256"],
                source="audit-verification",
                text=json.dumps(summary),
            )
            for path in sorted((directory / "originals/task").glob("*")):
                if path.suffix in (".txt", ".csv"):
                    body = path.read_text()
                    for index in range(0, len(body), 3000):
                        company(
                            args,
                            "attach_evidence",
                            run_id=args.run,
                            manifest_sha256=binding["sha256"],
                            source=f"task/{path.name}#chunk-{index // 3000}",
                            text=body[index : index + 3000],
                        )
            rows = task_evidence.read_csv(output / "decisions.csv", "employee_id")
            compact = [
                {k: v for k, v in row.items() if k != "evidence_ids"}
                for row in rows.values()
            ]
            company(
                args,
                "attach_evidence",
                run_id=args.run,
                manifest_sha256=binding["sha256"],
                source="verified-decisions",
                text=json.dumps(compact),
            )
            company(
                args,
                "attach_evidence",
                run_id=args.run,
                manifest_sha256=binding["sha256"],
                source="media-review",
                text=(directory / "media-review.json").read_text(),
            )
            print(
                f"Deterministic audit verified: {output}; validated artifact cache hit: {hit}"
            )
        result = api(
            args,
            "/api/runs",
            {
                "company_id": args.company,
                "run_id": args.run,
                "task": job["intake"]["idea"],
                "consent": False,
                "definition_of_done": job["intake"]["acceptance"].splitlines(),
            },
        )
        print(f"Started {result['id']}. Open the office or use tail {args.run}")
    elif args.command == "tail":
        tail(args)
    elif args.command == "projection":
        projection(args)
    elif args.command == "cancel":
        # Verify company before mutation.
        job_for(args)
        print(api(args, "/api/runs/" + args.run + "/cancel", {}))
    elif args.command == "finish":
        print(json.dumps(company(args, "finish", run_id=args.run), indent=2))
    elif args.command == "recall":
        print(json.dumps(company(args, "recall", target_run_id=args.run), indent=2))
    elif args.command == "capability":
        print(
            json.dumps(
                company(
                    args,
                    "capability",
                    manifest=json.loads(Path(args.manifest).read_text()),
                ),
                indent=2,
            )
        )
    elif args.command == "report":
        print(report(args))
    elif args.command == "daemon":
        daemon(args)
    else:
        print(json.dumps(company(args, args.command), indent=2))


if __name__ == "__main__":
    try:
        main()
    except urllib.error.HTTPError as error:
        sys.exit(f"Harness HTTP {error.code}: {error.read().decode()}")
    except (OSError, ValueError, StopIteration, subprocess.CalledProcessError) as error:
        sys.exit(str(error) or "Requested job was not found")
    except KeyboardInterrupt:
        sys.exit("Stopped CLI. A running job continues until explicitly cancelled.")
