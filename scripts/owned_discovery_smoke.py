"""Private HTTP discovery/retention proof. Synthetic packages, zero model calls."""

import hashlib
import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

import local_sqlite

REPO = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


with tempfile.TemporaryDirectory(prefix="gents-owned-discovery-") as temporary:
    root = Path(temporary).resolve()
    model = {
        "provider": "local",
        "model": "unavailable-discovery-fixture",
        "path": "models/not-installed.gguf",
        "context_tokens": 8192,
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Fixture Gent",
                "assignment": "unused",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 512,
        "planning_model_policy": {
            "failures_before_next": 2,
            "candidates": [
                {
                    "id": "missing",
                    "model": model,
                    "sha256": "0" * 64,
                    "minimum_memory_mib": 512,
                }
            ],
        },
    }
    (root / "config.json").write_text(json.dumps(config))
    (root / "scripts/fixtures").mkdir(parents=True)
    for name in (
        "mcp-owned-catalog.mjs",
        "mcp-component-probe.mjs",
        "mcp-component-store.mjs",
        "fixtures/mcp-reconciliation.mjs",
    ):
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    (root / "target/debug").mkdir(parents=True)
    shutil.copyfile(
        REPO / "target/debug/gents-rust-component",
        root / "target/debug/gents-rust-component",
    )
    # These are explicitly simulated metadata fixtures, never benchmark qualification.
    builder = r"""
import {publishComponentReview} from './scripts/mcp-component-store.mjs';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const root=pathToFileURL(process.cwd()+'/'), ids=[];
for(const company of ['discovery-owner','discovery-foreign']) {
 const source=`pub fn ${company.endsWith('owner')?'owned':'foreign'}_must_not_run()->u64{loop{}} ${company.endsWith('owner')?'pub fn owned_consume(x:u64,label:u64)->u64{let _=x;let _=label;loop{}}':''} // PRIVATE_SOURCE`;
 const source_sha256=createHash('sha256').update(source).digest('hex');
 const packet=await publishComponentReview(root,{'source.rs':source,'acceptance.rs':'PRIVATE_TEST',
  'qualification.json':JSON.stringify({fixture_simulated:true,conversation:'PRIVATE_CONVERSATION',proof:{status:'PASS',source_is_local_model_authored:true,source_sha256,provider_cost_usd:0}})},
  {company,fixture_id:'discovery-protocol-fixture',source_sha256});ids.push(packet.id);
}
console.log(JSON.stringify(ids));
"""
    env = {
        k: v
        for k, v in os.environ.items()
        if k
        in {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}
    }
    ids = json.loads(
        subprocess.check_output(
            ["node", "--input-type=module", "-e", builder], cwd=root, env=env, text=True
        )
    )
    http_port, model_port = free_port(), free_port()
    assert http_port != model_port
    env.update(
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{http_port}",
        CONFIG=str(root / "config.json"),
        RUN_STORE=str(root / "runs.sqlite3"),
        LOCAL_MODEL_PORT=str(model_port),
        GENTS_SQLITE_BRIDGE=str(REPO / "target/debug/gents-sqlite-bridge"),
        GENTS_LOCAL_ONLY="1",
        GENTS_DISABLE_SCHEDULER="1",
        GENTS_OFFICE_ACTIVITY="0",
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    base = f"http://127.0.0.1:{http_port}"

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req, timeout=35) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            error.add_note(error.read().decode())
            raise

    def rejected(path, body=None):
        try:
            request(path, body)
        except urllib.error.HTTPError as error:
            assert error.code == 400, error.code
            return
        raise AssertionError("Expected explicit rejection")

    process = subprocess.Popen(
        [str(REPO / "target/debug/gents-harness")],
        cwd=root,
        env=env,
        stdout=subprocess.DEVNULL,
    )
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None
                if attempt == 99:
                    raise AssertionError("Private discovery startup timed out")
                time.sleep(0.05)
        for company in ("discovery-owner", "discovery-foreign"):
            request(
                f"/api/companies/{company}",
                {"op": "register", "name": "Synthetic protocol fixture"},
            )
        # Change only the private fixture's owner to exercise the real HTTP filter.
        with local_sqlite.connect(
            root / "data/companies/discovery-foreign/brain.sqlite3"
        ) as db:
            db.execute("UPDATE company SET owner='different-private-owner'")
        catalog = request("/api/companies/discovery-owner/components")
        assert [entry["component_id"] for entry in catalog["components"]] == [ids[0]]
        assert (
            catalog["execution_authorized"] is False
            and catalog["promotion_authorized"] is False
        )
        for private in [
            ids[1],
            "PRIVATE_SOURCE",
            "PRIVATE_TEST",
            "PRIVATE_CONVERSATION",
            "loop{",
        ]:
            assert private not in json.dumps(catalog)
        assert "rejected" not in catalog
        rejected("/api/companies/discovery-foreign/components")
        rejected("/api/companies/missing-company/components")
        path = f"/api/companies/discovery-owner/planning/{uuid.uuid4()}"
        rejected(
            path,
            {
                "message": "No foreign catalog",
                "expected_revision": 0,
                "owned_component_ids": [ids[1]],
            },
        )
        state = request(
            path,
            {
                "message": "Discuss available interfaces only",
                "expected_revision": 0,
                "owned_component_ids": [ids[0]],
            },
        )
        assert (
            state["status"] == "failed"
            and state["inference_failure"]["inference_dispatched"] is False
        )
        assert (
            state["tools_executed"] is False
            and state["owned_component_catalog"]["components"] == catalog["components"]
        )
        retained = request(path)
        assert retained["owned_component_catalog"] == state["owned_component_catalog"]
        rejected(
            path,
            {
                "message": "Do not replace",
                "expected_revision": 1,
                "owned_component_ids": [ids[1]],
            },
        )
        assert request(path)["revision"] == 1
        # Add exact invocation data to the same retained conversation. Local
        # policy refusal again proves capture without dispatching inference.
        invocation = {
            "id": "call-one",
            "component_id": ids[0],
            "function": "owned_must_not_run",
            "arguments": [],
        }
        for mutation in [
            {**invocation, "actor": "forged"},
            {**invocation, "source": "forged"},
            {**invocation, "arguments": ["3"]},
            {**invocation, "component_id": ids[1]},
        ]:
            rejected(
                path,
                {
                    "message": "Reject unsafe preparation",
                    "expected_revision": 1,
                    "owned_invocations": [mutation],
                },
            )
        prepared = request(
            path,
            {
                "message": "Save the complete native output to result.json and read it back; use the supplied invocation unchanged",
                "expected_revision": 1,
                "owned_invocations": [invocation],
            },
        )
        assert (
            prepared["status"] == "failed"
            and prepared["inference_failure"]["inference_dispatched"] is False
        )
        assert prepared["owned_invocations"][0]["invocation"] == invocation
        assert prepared["source_inputs"][0] == {
            "id": "call-one",
            "tool": "owned_rust_call",
            "arguments": {"binding_id": "call-one", "arguments": []},
        }
        assert "PRIVATE_SOURCE" not in json.dumps(prepared)
        changed = {**invocation, "function": "different"}
        rejected(
            path,
            {
                "message": "Reject changed function",
                "expected_revision": 2,
                "owned_invocations": [changed],
            },
        )
        assert request(path)["revision"] == 2
        # Explicit simulated CEO verdict: HTTP contract/custody checks only.
        proposal = {
            "status": "ready",
            "question": "",
            "definition_of_done": ["Readback equals actual native output"],
            "assignments": [
                {
                    "actor": "Fixture Gent",
                    "task": "Obtain and save native output",
                    "tools": ["owned_rust_call", "files:read", "files:write"],
                    "acceptance": ["Exact native readback"],
                }
            ],
            "native_contracts": {
                "sources": [{"id": "call-one", "input": "call-one"}],
                "artifacts": [
                    {
                        "path": "result.json",
                        "source": "call-one",
                        "pointer": "/output",
                        "encoding": "json",
                    }
                ],
            },
        }
        prepared.update(status="ready", proposal=proposal)
        with local_sqlite.connect(
            root / "data/companies/discovery-owner/brain.sqlite3"
        ) as db:
            db.execute(
                "UPDATE ceo_planning SET state='ready',body=? WHERE id=?",
                (json.dumps(prepared), prepared["id"]),
            )
        definition_of_done = proposal["definition_of_done"]
        assert isinstance(definition_of_done, list)
        intake = {
            "idea": "Private custody protocol",
            "acceptance": definition_of_done[0],
            "exclusions": "remote services",
            "timeline": "private fixture",
            "company_signer": "fixture",
            "provider_signer": "fixture",
            "budget_usd": 0,
            "deliverable_type": "agent",
            "ceo_planning_id": prepared["id"],
            "ceo_planning_revision": 2,
        }
        run = str(uuid.uuid4())
        company_path = "/api/companies/discovery-owner"
        frozen = request(
            company_path, {"op": "intake", "run_id": run, "intake": intake}
        )
        job = next(job for job in frozen["jobs"] if job["run_id"] == run)
        assert job["state"] == "draft"
        spec = job["intake"]["required_owned_rust_calls"][0]
        assert spec["actor"] == "Fixture Gent" and spec["id"] == invocation["id"]
        assert (
            spec["source_sha256"] == prepared["owned_invocations"][0]["source_sha256"]
        )
        assert job["intake"]["ceo_owned_invocations"] == prepared["owned_invocations"]
        assert not (
            root / f"data/companies/discovery-owner/runs/{run}/workspace"
        ).exists()
        tampered = {
            **intake,
            "required_owned_rust_calls": [{**spec, "actor": "forged"}],
        }
        rejected(
            company_path,
            {"op": "intake", "run_id": str(uuid.uuid4()), "intake": tampered},
        )
        approved = request(
            company_path, {"op": "approve", "run_id": run, "digest": job["digest"]}
        )
        assert (
            next(job for job in approved["jobs"] if job["run_id"] == run)["state"]
            == "approved"
        )
        assert not (
            root / f"data/companies/discovery-owner/runs/{run}/workspace"
        ).exists()
        assert request("/api/runs")["runs"] == []
        # A second retained conversation uses a partial tuple. The simulated
        # CEO chooses an edge; only the final file is requested. Real HTTP
        # custody/freeze must add a receipt-only producer without file grants.
        edge_path = f"/api/companies/discovery-owner/planning/{uuid.uuid4()}"
        roots = [
            {
                "id": "root-value",
                "component_id": ids[0],
                "function": "owned_must_not_run",
                "arguments": [],
            },
            {
                "id": "final-value",
                "component_id": ids[0],
                "function": "owned_consume",
                "arguments": [None, "4"],
                "argument_slots": [0],
            },
        ]
        rejected(
            edge_path,
            {
                "message": "Invalid partial input",
                "expected_revision": 0,
                "owned_component_ids": [ids[0]],
                "owned_invocations": [roots[0], {**roots[1], "argument_slots": [1]}],
            },
        )
        edge_state = request(
            edge_path,
            {
                "message": "Connect the operations and save only final.json",
                "expected_revision": 0,
                "owned_component_ids": [ids[0]],
                "owned_invocations": roots,
            },
        )
        assert (
            edge_state["status"] == "failed"
            and edge_state["inference_failure"]["inference_dispatched"] is False
        )
        assert "PRIVATE_SOURCE" not in json.dumps(edge_state)
        child = next(
            m
            for m in edge_state["owned_invocations"]
            if m["invocation"]["id"] == "final-value"
        )
        assert "input_sha256" not in child and "input_template_sha256" in child
        edge_proposal = {
            "status": "ready",
            "question": "",
            "capability_mode": "native_prepared_graph",
            "definition_of_done": [
                "The final file matches the verified dependent result"
            ],
            "assignments": [
                {
                    "actor": "Fixture Gent",
                    "task": "Compute the typed chain and save its final result",
                    "tools": ["owned_rust_call", "files:read", "files:write"],
                    "acceptance": ["Exact native final readback"],
                }
            ],
            "native_contracts": {
                "sources": [
                    {
                        "id": "root-value",
                        "input": "root-value",
                        "actor": "Fixture Gent",
                        "argument_sources": [],
                    },
                    {
                        "id": "final-value",
                        "input": "final-value",
                        "actor": "Fixture Gent",
                        "argument_sources": [
                            {
                                "field": "arguments",
                                "source": "root-value",
                                "pointer": "/output",
                                "argument_index": 0,
                                "value_pointer": "",
                            }
                        ],
                    },
                ],
                "artifacts": [
                    {
                        "path": "final.json",
                        "source": "final-value",
                        "writer": "Fixture Gent",
                        "pointer": "/output",
                        "encoding": "json",
                    }
                ],
            },
        }
        edge_state.update(status="ready", proposal=edge_proposal)
        with local_sqlite.connect(
            root / "data/companies/discovery-owner/brain.sqlite3"
        ) as db:
            db.execute(
                "UPDATE ceo_planning SET state='ready',body=? WHERE id=?",
                (json.dumps(edge_state), edge_state["id"]),
            )
        edge_definition_of_done = edge_proposal["definition_of_done"]
        assert isinstance(edge_definition_of_done, list)
        edge_intake = {
            **intake,
            "idea": "Private dependent custody protocol",
            "acceptance": edge_definition_of_done[0],
            "ceo_planning_id": edge_state["id"],
            "ceo_planning_revision": 1,
        }
        edge_run = str(uuid.uuid4())
        edge_frozen = request(
            company_path, {"op": "intake", "run_id": edge_run, "intake": edge_intake}
        )
        edge_job = next(j for j in edge_frozen["jobs"] if j["run_id"] == edge_run)
        bindings = edge_job["intake"]["artifact_bindings"]
        assert len(bindings) == 2
        receipt = next(b for b in bindings if b.get("receipt_only"))
        published = [b for b in bindings if not b.get("receipt_only")]
        assert len(published) == 1 and published[0]["path"] == "final.json"
        spec = next(
            s
            for s in edge_job["intake"]["required_owned_rust_calls"]
            if s["id"] == "final-value"
        )
        assert "input_sha256" not in spec and spec["input_recipe"]["arguments"] == [
            None,
            "4",
        ]
        assert spec["input_recipe"]["sources"][0]["path"] == receipt["path"]
        assert not (
            root / f"data/companies/discovery-owner/runs/{edge_run}/workspace"
        ).exists()
        assert request("/api/runs")["runs"] == []
        # Simulated local draft, real owner HTTP adoption and frozen provenance.
        # No model or candidate can run in this deliberately unavailable profile.
        selection_path = f"/api/companies/discovery-owner/planning/{uuid.uuid4()}"
        selected = request(
            selection_path,
            {
                "message": "Propose local calls over this data",
                "expected_revision": 0,
                "owned_component_ids": [ids[0]],
                "owned_argument_data": {"label": "9"},
            },
        )
        assert (
            selected["owned_argument_data"] == {"label": "9"}
            and selected["source_inputs"] == []
        )
        assert selected["inference_failure"]["inference_dispatched"] is False
        rejected(
            selection_path,
            {
                "message": "Change the retained value",
                "expected_revision": 1,
                "owned_argument_data": {"label": "10"},
            },
        )
        assert request(selection_path)["revision"] == 1
        selected_calls = [
            {
                "id": "chosen-root",
                "component_id": ids[0],
                "function": "owned_must_not_run",
                "arguments": [],
            },
            {
                "id": "chosen-final",
                "component_id": ids[0],
                "function": "owned_consume",
                "arguments": [None, "9"],
                "argument_slots": [0],
            },
        ]
        raw_calls = [
            {
                "id": "chosen-root",
                "component_id": ids[0],
                "function": "owned_must_not_run",
                "arguments": [],
            },
            {
                "id": "chosen-final",
                "component_id": ids[0],
                "function": "owned_consume",
                "arguments": [
                    {"kind": "result", "source": "chosen-root", "pointer": ""},
                    {"kind": "data", "data": "label", "pointer": ""},
                ],
            },
        ]
        basis = {
            "kind": "owned_selection_v1",
            "session": selected["id"],
            "revision": 1,
            "catalog_sha256": selected["owned_component_catalog"]["catalog_sha256"],
            "data_sha256": hashlib.sha256(b'{"label":"9"}').hexdigest(),
            "proposal": raw_calls,
        }
        proposal_sha = hashlib.sha256(
            json.dumps(
                basis, sort_keys=True, separators=(",", ":"), ensure_ascii=False
            ).encode()
        ).hexdigest()
        selected["messages"].append(
            {
                "role": "ceo",
                "message": {
                    "status": "selection_review",
                    "owned_selection_proposal": raw_calls,
                },
            }
        )
        selected["pending_owned_selection"] = {
            "proposal": raw_calls,
            "proposal_sha256": proposal_sha,
            "data_sha256": basis["data_sha256"],
            "catalog_sha256": basis["catalog_sha256"],
            "invocations": selected_calls,
            "execution_authorized": False,
            "promotion_authorized": False,
        }
        with local_sqlite.connect(
            root / "data/companies/discovery-owner/brain.sqlite3"
        ) as db:
            db.execute(
                "UPDATE ceo_planning SET body=? WHERE id=?",
                (json.dumps(selected), selected["id"]),
            )
        adopted = request(
            selection_path,
            {
                "message": "Prepare these reviewed calls; plan the final output next",
                "expected_revision": 1,
                "owned_invocations": selected_calls,
            },
        )
        assert adopted["accepted_owned_selection"]["proposal"] == raw_calls
        assert adopted["accepted_owned_selection"]["owner_confirmed_revision"] == 2
        assert adopted["accepted_owned_selection"]["proposal_sha256"] == proposal_sha
        with local_sqlite.connect(
            root / "data/companies/discovery-owner/brain.sqlite3"
        ) as db:
            boundaries = [
                json.loads(row[0])
                for row in db.execute(
                    "SELECT body FROM audit WHERE run_id=? AND kind='ceo_owned_selection_adopted'",
                    (adopted["id"],),
                )
            ]
        assert boundaries == [
            {
                "kind": "owned_selection_adopted",
                "start_revision": 2,
                "proposal_sha256": proposal_sha,
                "data_sha256": adopted["accepted_owned_selection"]["data_sha256"],
                "catalog_sha256": adopted["accepted_owned_selection"]["catalog_sha256"],
            }
        ]
        assert (
            adopted.get("pending_owned_selection") is None
            and len(adopted["owned_invocations"]) == 2
        )
        assert "PRIVATE_SOURCE" not in json.dumps(adopted)
        rejected(
            selection_path,
            {
                "message": "Append a call after adoption",
                "expected_revision": 2,
                "owned_invocations": selected_calls
                + [
                    {
                        "id": "extra",
                        "component_id": ids[0],
                        "function": "owned_must_not_run",
                        "arguments": [],
                    }
                ],
            },
        )
        assert request(selection_path)["revision"] == 2
        selection_plan = json.loads(
            json.dumps(edge_proposal)
            .replace("root-value", "chosen-root")
            .replace("final-value", "chosen-final")
        )
        adopted.update(status="ready", proposal=selection_plan)
        with local_sqlite.connect(
            root / "data/companies/discovery-owner/brain.sqlite3"
        ) as db:
            db.execute(
                "UPDATE ceo_planning SET state='ready',body=? WHERE id=?",
                (json.dumps(adopted), adopted["id"]),
            )
        selection_run = str(uuid.uuid4())
        selection_intake = {
            **intake,
            "ceo_planning_id": adopted["id"],
            "ceo_planning_revision": 2,
            "acceptance": selection_plan["definition_of_done"][0],
        }
        for edited in [
            {"ceo_owned_argument_data": {"label": "10"}},
            {"ceo_owned_selection": {"proposal": "forged"}},
        ]:
            rejected(
                company_path,
                {
                    "op": "intake",
                    "run_id": str(uuid.uuid4()),
                    "intake": {**selection_intake, **edited},
                },
            )
        selected_frozen = request(
            company_path,
            {"op": "intake", "run_id": selection_run, "intake": selection_intake},
        )
        selected_job = next(
            j for j in selected_frozen["jobs"] if j["run_id"] == selection_run
        )
        assert selected_job["intake"]["ceo_owned_argument_data"] == {"label": "9"}
        assert selected_job["intake"]["ceo_owned_selection"]["proposal"] == raw_calls
        assert request("/api/runs")["runs"] == []
        print(
            json.dumps(
                {
                    "status": "PASS",
                    "scope": "synthetic HTTP discovery protocol, not task competence",
                    "owner_filter": True,
                    "immutable_capture": True,
                    "owner_preparation_and_frozen_intake": True,
                    "ceo_verdict_simulated": True,
                    "typed_template_freeze": True,
                    "receipt_only_intermediate": True,
                    "owned_selection_adoption_and_provenance": True,
                    "candidate_invocations": 0,
                    "model_calls": 0,
                    "provider_cost_usd": 0,
                }
            )
        )
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)
        # Only this private test tree; never follow aliases while removing sealed packages.
        for parent, dirs, files in os.walk(root, followlinks=False):
            os.chmod(parent, 0o700)
