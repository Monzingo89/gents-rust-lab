#!/usr/bin/env python3
"""Poll a dedicated allowlisted inbox. Inference and action approval stay in Rust."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import time
import uuid
import company_tools
import gents


def poll(args):
    marker = company_tools.confined(company_tools.ROOT, "data/company-inbox.json")
    marker.parent.mkdir(exist_ok=True)
    seen = json.loads(marker.read_text()) if marker.exists() else {}
    result = company_tools.mailbox({})
    messages = result.get("messages", [])
    state = gents.company(args, "status")
    if any(job["state"] == "running" for job in state["jobs"]):
        return
    for message in reversed(messages):
        identity = str(message.get("message_id") or message.get("id"))
        key = hashlib.sha256(identity.encode()).hexdigest()
        if key in seen:
            continue
        if os.getenv("GENTS_EMAIL_PROVIDER") == "resend":
            message = company_tools.mailbox({"id": message["id"]})
        text = message.get("text", "")
        if not isinstance(text, str) or not text.strip():
            continue
        sender = message.get("from", "")
        task = f"Email request from {sender}\nSubject: {message.get('subject', '')}\n\n{text}"[:9000]
        # A stable run ID plus persisted sender policy makes intake retries inspectable.
        run = str(uuid.uuid5(uuid.NAMESPACE_URL, "gents-inbox:" + key))
        existing = next((j for j in state["jobs"] if j["run_id"] == run), None)
        intake = {"idea": task, "acceptance": "Complete the email request with actual evidence\nReport the result and any unresolved limits",
                  "deliverable_type": "agent", "exclusions": "Unrequested spending, secret disclosure, cross-company access",
                  "timeline": "Continue until done or cancelled", "company_signer": "Configured local inbox sender policy",
                  "provider_signer": "Gents company", "budget_usd": 0, "signature_status": "unsigned", "source_email": identity}
        if existing is None:
            state = gents.company(args, "intake", run_id=run, intake=intake)
            existing = next(j for j in state["jobs"] if j["run_id"] == run)
        if existing["state"] == "draft":
            gents.company(args, "approve", run_id=run, digest=existing["digest"])
        if existing["state"] in ("approved", "draft"):
            gents.api(args, "/api/runs", {"company_id": args.company, "run_id": run, "task": task,
                                         "consent": False, "definition_of_done": existing["intake"]["acceptance"].splitlines()})
        seen[key] = {"run_id": run, "received_at": int(time.time())}
        temporary = marker.with_suffix(".tmp")
        temporary.write_text(json.dumps(seen))
        temporary.replace(marker)
        print(f"Email request admitted as company run {run}; admin review follows.", flush=True)
        break


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default="http://127.0.0.1:8788")
    parser.add_argument("--company", default="we-need-gents-lc")
    args = parser.parse_args()
    while True:
        try:
            poll(args)
        except Exception as error:
            # Email/provider errors may include authentication details; log class only.
            print(f"Email listener waiting for configuration or service recovery ({type(error).__name__}).", flush=True)
        time.sleep(30)
