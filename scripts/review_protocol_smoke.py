"""Real HTTP keyed QA retries; simulated judgments, not model competence."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid

REPO = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="gents-review-protocol-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    for name in ["company_tools.py", "public_network.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(REPO / "config/public-network-policy.json", root / "config/public-network-policy.json")
    model = {"provider": "mock", "model": "protocol-bootstrap"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Engineer Gent", "assignment": "fixture", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512, "max_run_seconds": 120}
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(root / "config.json"),
               RUN_STORE=str(root / "runs.sqlite3"), GENTS_DISABLE_SCHEDULER="1", GENTS_LOCAL_ONLY="1")
    env.pop("RUST_OPENAI_API_KEY", None)
    process = None

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait_for(action, predicate):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                value = action()
                if predicate(value): return value
            except OSError:
                pass
            time.sleep(.1)
        raise AssertionError("QA recovery fixture timed out")

    def start():
        child = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env,
                                 stdout=subprocess.DEVNULL)
        try:
            wait_for(lambda: request("/api/config"), bool)
            return child
        except Exception:
            child.terminate(); child.wait(timeout=15)
            raise

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    try:
        for scenario in ["repair","always_bad","source_change"]:
            always_bad=scenario=="always_bad"
            config["admin_judge"]=model
            config["agents"][0].update(planner=model,worker=model)
            (root/"config.json").write_text(json.dumps(config))
            process=start()
            request("/api/companies/one",{"op":"register","name":"Protocol fixture"})
            run=str(uuid.uuid4());path=f"/api/runs/{run}"
            intake={"idea":"qa-protocol-private-fixture","acceptance":"Require a complete independent protocol response","exclusions":"paid services","timeline":"fixture","company_signer":"Fixture","provider_signer":"Fixture","budget_usd":0,"deliverable_type":"agent"}
            state=request("/api/companies/one",{"op":"intake","run_id":run,"intake":intake})
            digest=next(j["digest"] for j in state["jobs"] if j["run_id"]==run)
            request("/api/companies/one",{"op":"approve","run_id":run,"digest":digest})
            request("/api/runs",{"company_id":"one","run_id":run,"task":intake["idea"],"definition_of_done":[intake["acceptance"]],"consent":False})
            wait_for(lambda:request(path+"/attention"),lambda x:x.get("attention"))
            request(path+"/cancel",{});wait_for(lambda:request(path),lambda x:x["status"]=="cancelled");stop()
            work=root/f"data/companies/one/runs/{run}";checkpoint=work/"autonomy.json"
            state=json.loads(checkpoint.read_text())
            # Only this disposable test fixture is seeded, never a benchmark.
            state.update(attention=None,review_intent=None,recovery_attempts=0,repairs_since_progress=11 if always_bad else 0,plan_queue=[{"action":"finish","arguments":{"deliverable":"SIMULATED protocol fixture","evidence":"No external effects claimed"}}])
            checkpoint.write_text(json.dumps(state))
            special={"provider":"mock","model":"mock-review-protocol-always-invalid" if always_bad else "mock-review-protocol"}
            config["admin_judge"]=special;config["agents"][0].update(planner=special,worker=special)
            (root/"config.json").write_text(json.dumps(config))
            boundary=len(json.loads((work/"checkpoint.json").read_text())["events"])
            process=start();request(path+"/resume",{})
            if scenario=="source_change":
                def second_review_started(snapshot):
                    events=snapshot["events"][boundary:]
                    return any(e["kind"]=="completion_review_protocol" for e in events) and sum(e["kind"]=="call_started" and e["role"]=="judge" for e in events)>=2
                wait_for(lambda:request(path),second_review_started)
                (work/"workspace").mkdir(exist_ok=True)
                (work/"workspace/changed.txt").write_text("Disposable fixture edit during QA")
                saved=wait_for(lambda:request(path),lambda x:any(e["kind"]=="retry_scheduled" for e in x["events"][boundary:]))
                assert saved["status"]!="completed"
                assert not any(e["kind"]=="completion_review" and json.loads(e["message"])["verdict"]=="accepted" for e in saved["events"][boundary:])
                checkpoint_value=json.loads(checkpoint.read_text())
                assert any("workspace artifacts changed during QA" in item.get("executive_recovery",{}).get("error","") for item in checkpoint_value["history"])
                assert not list(work.glob("tool-*.json"))
                request(path+"/cancel",{});wait_for(lambda:request(path),lambda x:x["status"]=="cancelled")
                stop()
                continue
            if always_bad:
                attention=wait_for(lambda:request(path+"/attention"),lambda x:x.get("attention"))["attention"]
                assert "12 recovery attempts" in attention["question"]
                saved=request(path);assert saved["status"]!="completed"
            else:
                saved=wait_for(lambda:request(path),lambda x:x["status"]=="completed")
            events=saved["events"][boundary:]
            protocols=[json.loads(e["message"]) for e in events if e["kind"]=="completion_review_protocol"]
            assert len(protocols)==(3 if always_bad else 2),protocols
            assert protocols[0]["protocol"]["valid"] is False
            assert "Missing criterion key 0" in protocols[0]["protocol"]["errors"]
            assert protocols[-1]["protocol"]["valid"] is (not always_bad)
            assert sum(e["kind"]=="call_started" and e["role"]=="judge" for e in events)==len(protocols)
            assert not any(e["kind"]=="call_started" and e["role"]=="worker" for e in events)
            assert not list(work.glob("tool-*.json"))
            if always_bad:
                request(path+"/cancel",{});wait_for(lambda:request(path),lambda x:x["status"]=="cancelled")
            stop()
        print("PASS: malformed positive QA verdict never accepted; bounded reviewer-only retry gets fresh keyed judgment without author/tool calls; repeated invalid protocol exhausts three attempts; source changes during retry reject stale acceptance; simulated semantics")
    finally:
        if process is not None and process.poll() is None:
            process.terminate();process.wait(timeout=15)
