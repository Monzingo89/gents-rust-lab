#!/usr/bin/env python3
"""Typed company tools. Called only after a recorded admin-judge decision.

Prompts/web/email are untrusted data. Secrets never enter model context. Shells
run in a per-run macOS sandbox; model downloads never execute repository code.
"""

import argparse
import datetime as dt
import email
import hashlib
import http.client
import imaplib
import json
import os
import re
import shutil
import signal
import smtplib
import socket
import ssl
import subprocess
import sys
import time
import urllib.parse
import xml.etree.ElementTree as ET
from email import utils as email_utils
from email.message import EmailMessage
from email.policy import default
from html.parser import HTMLParser
from pathlib import Path

from public_network import public_address
from public_research import redact_public_at_tokens

ROOT = Path(__file__).resolve().parents[1]
ACTIVE_CHILD = None
TOOLS = {
    "models_inventory": "Read bounded offline model identity and context metadata; no loading, network, code execution or automatic admission",
    "integer_arithmetic": "calculations:[{id,values:[signed integers],subtract:signed integer}]; exact bounded sums and differences with no side effects",
    "web_search": "query: public search string",
    "web_read": "url: public HTTPS page; returns source URL and extracted text",
    "browser": "url; actions: [{kind:click|fill, selector, value?}]; screenshot captured",
    "files": "op: list|read|write; path: relative to run workspace; content for write",
    "pdf_inspect": "path:existing workspace PDF; page_start>=1; page_count:1..8; extract text, hash and render pages for independent review",
    "pdf_render": "source:agent-authored workspace JSON page specification; output:workspace PDF; deterministic text and vector rendering without shell or pdftk",
    "image_review": "paths:1..2 workspace PNGs, question, optional max_tokens:64..1024; local hash-bound vision observations under the shared model lease",
    "shell": "command: run code/tests in isolated run workspace; timeout_seconds up to 3600; no network or home access",
    "resources": "Inspect free disk, system RAM, local model files, process usage, and secret-safe configured-integration status loaded from gitignored .env.local/.env.company",
    "skill_compose": "id, purpose, roles:[planner|worker], steps:[existing tool ids], instructions; save a declarative composition without expanding permissions",
    "skill_list": "List declarative skills composed in this run",
    "skill_read": "id; retrieve approved instructions (including document-workflow) or a run composition, without expanding permissions",
    "source_read": "path: allowlisted repository-relative source file; returns bounded content and SHA-256",
    "source_patch": "path, expected_sha256, content; atomic hash-guarded replacement with backup, fixed checks, and rollback",
    "harness_restart": "reason; handled by the Rust supervisor after durable checkpointing",
    "hf_dataset_search": "query: search datasets visible to the configured server-side HF_TOKEN",
    "hf_dataset_inspect": "repo: owner/name; optional revision; return immutable revision and PDF inventory",
    "hf_dataset_files": "repo, revision, optional query, offset, limit:1..20; search/page this run's inspected source file inventory locally",
    "hf_dataset_download": "repo: owner/name; revision: immutable commit; files: exact PDF paths; destination: run-workspace folder",
    "models_search": "query: Hugging Face model search",
    "model_download": "repo: owner/name; revision: 40-character commit; files: exact GGUF/Safetensors/config/tokenizer paths; destination: folder under ./models",
    "model_quantize": "source: GGUF path relative to ./models; destination: new GGUF path; type: Q4_K_M|Q5_K_M|Q3_K_M|Q2_K",
    "model_select": "path: GGUF or MLX model directory under ./models; provider: local|mlx; model: display name; applies to current run worker",
    "email_list": "Read allowed-sender messages from dedicated IMAP mailbox or Resend inbound; optional id for body",
    "email_send": "to, subject, text; only configured allowed recipients; idempotency_key supplied by runtime",
    "email_status": "id: sent Resend email UUID; read actual delivery or scheduled status",
    "schedule": "op:create|update|list|pause|resume; create/update needs id, time:HH:MM (preparation start), timezone:IANA, task, acceptance:newline criteria. Only original creating run may update after admin review. Daily local agent jobs, not automatic report text.",
    "training_prepare": "role, base_model (models/... Safetensors directory), iterations, samples: {train,valid,test} with reviewed provenance; registers a dataset for training",
    "training_run": "training_id from training_prepare; releases inference before acquiring shared GPU training queue",
    "training_status": "training_id; actual training progress and held-out metrics",
    "process_stop": "pid and identity (from resources); requires explicit browser confirmation for a non-company process",
    "ask_user": "question: missing information or access required; browser pauses and resumes the same run",
    "finish": "deliverable: final response; evidence: artifact paths or actual tool results; QA checks every criterion",
}


def confined(base, relative):
    path = Path(relative)
    if path.is_absolute() or any(p in ("..", ".") for p in path.parts):
        raise ValueError("use a relative path without traversal")
    target = base / path
    cursor = base
    if base.is_symlink():
        raise ValueError("symlink root forbidden")
    for part in path.parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError("symlink paths forbidden")
    if not target.resolve().is_relative_to(base.resolve()):
        raise ValueError("path escapes scope")
    return target


SOURCE_PREFIXES = ("crates/", "scripts/", "web/", "config/", "docs/")
SOURCE_SUFFIXES = {".rs", ".py", ".js", ".mjs", ".json", ".md", ".css", ".html"}
# Frozen acceptance, launch/admission code and authority readers are host-owned.
# New reviewed curriculum helpers inherit this boundary without a filename list.
PROTECTED_SOURCE_PREFIXES = ("scripts/fixtures/", "scripts/mcp-", "config/company.mcp-")
PROTECTED_SOURCE_FILES = {
    "crates/harness/src/auth.rs",
    "crates/harness/src/security.rs",
    "crates/harness/src/provider.rs",
    "crates/harness/src/escalation.rs",
    "crates/harness/src/main.rs",
    "crates/harness/src/task_frontdoor.rs",
    "crates/harness/src/ceo_fastpath.rs",
    "crates/harness/src/ceo_planning.rs",
    "crates/harness/src/reasoning_planning.rs",
    "web/assets/ceo-execution.js",
    "scripts/task_frontdoor_smoke.py",
    "scripts/task_frontdoor_browser.mjs",
    "scripts/company-env.mjs",
    "config/global-pstc.json",
    "scripts/run-harness.mjs",
    "scripts/company_tools.py",
    "scripts/vision_tools.py",
    "config/vision.local.json",
    "scripts/company_research.py",
    "scripts/local-showcase-task.py",
    "scripts/showcase_observer.py",
    "scripts/test_showcase_observer.py",
    "scripts/fixtures/showcase-observer/company_tests.py",
    "scripts/fixtures/showcase-observer/company_quality.py",
    "scripts/inference_budget_smoke.py",
    "scripts/rename_reconciliation_smoke.py",
    "scripts/experiment_loop.py",
    "scripts/test_experiment_loop.py",
    "scripts/experiment_loop_smoke.py",
    "scripts/experiment_evaluation.py",
    "scripts/experiment_continuation.py",
    "scripts/research_agenda.py",
    "scripts/test_research_agenda.py",
    "config/research-agenda.example.json",
    "scripts/test_experiment_evaluation.py",
    "scripts/experiment_evaluation_smoke.py",
    "config/experiment-loop.example.json",
    "config/experiment-loop.rust-example.json",
    "config/experiment-loop.repair-example.json",
    "config/experiment-loop.focused-example.json",
    "config/company-research.json",
    "scripts/public_research.py",
    "scripts/python_snapshot_check.py",
    "scripts/test_python_snapshot_check.py",
    "scripts/document_renderer.py",
    "crates/harness/src/governance.rs",
    "crates/harness/src/company.rs",
    "crates/harness/src/autonomy.rs",
    "crates/harness/src/inference_budget.rs",
    "crates/harness/src/python_review_submission.rs",
    "crates/harness/src/contracts.rs",
    "crates/harness/src/local_runtime.rs",
    "crates/harness/src/skills.rs",
    "crates/harness/src/local_ingress.rs",
    "crates/harness/src/operator_answers.rs",
    "scripts/company-browser.mjs",
    "scripts/public-network.mjs",
    "scripts/public_network.py",
    "scripts/browser-public-http.mjs",
    "config/public-network-policy.json",
    "scripts/test_public_network.py",
    "scripts/public-network.test.mjs",
    "scripts/fixtures/public-addresses.json",
    "crates/harness/src/runtime_pool.rs",
    "crates/harness/src/runtime_environment.rs",
    "crates/harness/src/runtime_fit.rs",
    "crates/harness/src/local_generation.rs",
    "scripts/model_security_scan.py",
    "scripts/model_security_inspection.py",
    "scripts/model-security-service.mjs",
    "crates/harness/src/memory.rs",
    "crates/harness/src/memory_archive.rs",
    "crates/harness/src/semantic_memory.rs",
    "crates/harness/src/memory_consumption.rs",
    "crates/harness/src/memory_handoff.rs",
    "crates/harness/src/workflow.rs",
    "crates/harness/src/lease_clock.rs",
    "crates/harness/src/context.rs",
    "crates/harness/src/review_context.rs",
    "crates/harness/src/completion_review.rs",
    "crates/harness/src/authorization_context.rs",
    "crates/harness/src/rust_review.rs",
    "crates/harness/src/rust_workflow.rs",
    "crates/harness/src/worker_packet.rs",
    "crates/harness/src/recovery_scope.rs",
    "crates/harness/src/campaign.rs",
    "scripts/paired_campaign.py",
    "scripts/local_sqlite.py",
    "scripts/test_local_sqlite.py",
    "scripts/company_schedule.py",
    "scripts/mcp-curriculum-journal.py",
    "scripts/mcp-coding-verifier.mjs",
    "scripts/mcp-workflow-verifier.mjs",
    "scripts/mcp-development-curriculum.mjs",
    "scripts/mcp-team-verification.mjs",
    "config/mcp-development-curriculum.json",
    "scripts/model_inventory.py",
    "scripts/model_provenance.py",
    "config/enterprise-model-roster.json",
    "crates/harness/src/role_probe.rs",
    "config/enterprise-role-probes.json",
    "scripts/role_probe_campaign.py",
    "crates/harness/src/enterprise.rs",
    "crates/harness/src/banking.rs",
    "crates/harness/src/release_gate.rs",
    "crates/harness/src/release_policy.rs",
    "scripts/enterprise_campaign.py",
    "config/enterprise-model-candidates.json",
    "config/company-tools.json",
    "config/company-tool-inputs.json",
}


def source_target(relative):
    if not isinstance(relative, str) or not relative.startswith(SOURCE_PREFIXES):
        raise ValueError("source path is outside the repair allowlist")
    if Path(relative).as_posix() != relative:
        raise ValueError("source path must use its canonical relative spelling")
    if any(
        part.startswith(".env")
        or part in {"data", "models", "target", ".git", "node_modules"}
        for part in Path(relative).parts
    ):
        raise ValueError("secret, data, model, build, and VCS paths are forbidden")
    # macOS commonly resolves case aliases to the same file. Match protected
    # paths before I/O, and reject separator/dot aliases instead of normalizing
    # an untrusted model path into a privileged target.
    if relative.casefold() in {
        path.casefold() for path in PROTECTED_SOURCE_FILES
    } or relative.casefold().startswith(PROTECTED_SOURCE_PREFIXES):
        raise ValueError(
            "authentication, provider, secret, and security-boundary files require human review"
        )
    target = confined(ROOT, relative)
    if target.exists() and target.stat().st_nlink != 1:
        raise ValueError("hard-linked source files require human review")
    if target.suffix.lower() not in SOURCE_SUFFIXES or target.is_symlink():
        raise ValueError("source file type or symlink is not repairable")
    return target


def source_checks(path, work):
    relative = str(path.relative_to(ROOT))
    if relative.startswith("crates/"):
        return command(
            [
                shutil.which("cargo") or "cargo",
                "test",
                "--manifest-path",
                str(ROOT / "Cargo.toml"),
                "-p",
                "gents-harness",
            ],
            work,
            timeout=1800,
        )
    if (
        relative == "scripts/company_tools.py"
        or relative == "scripts/test_company_tools.py"
    ):
        return command(
            [sys.executable, str(ROOT / "scripts/test_company_tools.py")],
            work,
            timeout=300,
        )
    if relative.startswith("web/") or relative.endswith(".mjs"):
        return command(
            [
                shutil.which("node") or "node",
                "--test",
                str(ROOT / "scripts/gent-avatars.test.mjs"),
            ],
            work,
            timeout=300,
        )
    if path.suffix.lower() == ".json":
        try:
            json.loads(path.read_text())
            return {"exit_code": 0, "output": "JSON parsed successfully"}
        except Exception as error:
            return {"exit_code": 1, "output": f"JSON validation failed: {error}"}
    return {
        "exit_code": 0,
        "output": "No executable check required for this documentation-only file",
    }


def tls_context():
    context = ssl.create_default_context()
    if Path("/etc/ssl/cert.pem").is_file():
        context.load_verify_locations("/etc/ssl/cert.pem")
    return context


class PinnedHTTPS(http.client.HTTPSConnection):
    def __init__(self, *args, **kwargs):
        # HTTPSConnection initializes _context itself; assigning it beforehand
        # loses the caller's configured certificate store and mocked transport.
        context = kwargs.pop("context", None)
        super().__init__(*args, context=context, **kwargs)

    def connect(self):
        # Pin the validated address to prevent DNS rebinding between validation/connect.
        sock = socket.create_connection(
            (str(public_address(self.host)), self.port), self.timeout
        )
        self.sock = self._context.wrap_socket(sock, server_hostname=self.host)


def web_request(url, method="GET", payload=None, headers=None, limit=2_000_000):
    for _ in range(6):
        parsed = urllib.parse.urlsplit(url)
        if (
            parsed.scheme != "https"
            or not parsed.hostname
            or parsed.username
            or parsed.password
            or parsed.port not in (None, 443)
        ):
            raise ValueError(
                "public HTTPS on port 443 required, without embedded credentials"
            )
        connection = PinnedHTTPS(parsed.hostname, timeout=40, context=tls_context())
        try:
            connection.request(
                method,
                urllib.parse.urlunsplit(("", "", parsed.path or "/", parsed.query, "")),
                body=payload,
                headers={"User-Agent": "GentsCompany/1.0", **(headers or {})},
            )
            response = connection.getresponse()
            if response.status in (301, 302, 303, 307, 308):
                if headers:
                    raise ValueError(
                        "requests with caller-supplied headers cannot redirect"
                    )
                url = urllib.parse.urljoin(url, response.getheader("Location", ""))
                continue
            data = response.read(limit + 1)
            if len(data) > limit:
                raise ValueError("response exceeded size limit")
            if not 200 <= response.status < 300:
                # Never print API responses that could echo credentials/content.
                raise ValueError(f"remote service returned HTTP {response.status}")
            return url, data
        finally:
            connection.close()
    raise ValueError("too many redirects")


class PageText(HTMLParser):
    def __init__(self):
        super().__init__()
        self.skip = 0
        self.text = []
        self.links = []

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style", "noscript"):
            self.skip += 1
        if tag == "a":
            href = dict(attrs).get("href", "")
            assert href is not None
            if href.startswith("//duckduckgo.com/l/?"):
                href = urllib.parse.parse_qs(
                    urllib.parse.urlsplit("https:" + href).query
                ).get("uddg", [href])[0]
            if href.startswith("https://"):
                self.links.append(href)

    def handle_endtag(self, tag):
        if tag in ("script", "style", "noscript"):
            self.skip = max(0, self.skip - 1)

    def handle_data(self, data):
        if not self.skip and data.strip():
            self.text.append(data.strip())


def public_research_view(payload):
    # The company-authored pure view removes ambiguous contact/decorator tokens.
    # Native privacy checks still assess the complete result after this adapter.
    cleaned, count = redact_public_at_tokens(payload)
    if not isinstance(cleaned, dict):
        raise TypeError("Public research payload must be an object")
    cleaned["public_view"] = {
        "method": "whole_at_sign_tokens_removed",
        "tokens_removed": count,
        "lossy": count > 0,
        "scope": "Public research excerpt only; removed tokens have no mapping. Not exact source code or complete de-identification; native privacy checks still apply.",
    }
    if len(json.dumps(cleaned).encode("utf-8")) > 96000:
        raise ValueError(
            "Public research view exceeds its bounded output budget; use a narrower source"
        )
    return cleaned


def relevant_search_results(query, results):
    # Some public RSS search responses silently ignore the query and return
    # generic news homepages. Do not present those as topical research evidence.
    stop = {
        "the",
        "and",
        "for",
        "with",
        "from",
        "about",
        "latest",
        "new",
        "news",
        "current",
        "recent",
        "today",
        "breaking",
        "updates",
        "related",
        "anything",
        "companies",
        "company",
        "products",
        "product",
        "announcements",
    }
    terms = {
        term.casefold() for term in re.findall(r"[\w-]+", query) if len(term) >= 2
    } - stop
    if "ai" in terms:
        terms.update(("artificial", "intelligence"))
    if not terms:
        raise ValueError(
            "Search query is too broad; include a specific subject or source"
        )
    matches = [
        row
        for row in results
        if any(
            re.search(
                r"\b" + re.escape(term) + r"\b", " ".join(row.values()), re.IGNORECASE
            )
            for term in terms
        )
    ]
    if not matches:
        raise ValueError(
            "Public search returned only off-topic results, not evidence for this query. Narrow the topic/source query or use web_read on known official sources. Do not invent findings from these results."
        )
    return matches


def command(args, cwd, env=None, timeout=300):
    global ACTIVE_CHILD
    # Disk-backed logs avoid unlimited output in memory; return a bounded tail.
    log = cwd / ".tool-output.log"
    with log.open("wb") as stream:
        process = subprocess.Popen(
            args,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=stream,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        ACTIVE_CHILD = process
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code = -1
        ACTIVE_CHILD = None
    with log.open("rb") as stream:
        stream.seek(max(0, log.stat().st_size - 18000))
        output = stream.read().decode("utf-8", errors="replace")
    return {"exit_code": code, "output": output}


def memory_command(args, cwd, env, timeout=1800):
    """Keep image-model output off disk until the caller's data assessment."""
    global ACTIVE_CHILD
    process = subprocess.Popen(
        args,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    ACTIVE_CHILD = process
    try:
        try:
            output, error = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                output, error = process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                output, error = process.communicate()
            raise ValueError("local vision process exceeded its bounded runtime")
        if len(output) > 2 * 1024 * 1024 or len(error) > 2 * 1024 * 1024:
            raise ValueError("local vision process output exceeded 2 MiB")
        return {
            "exit_code": process.returncode,
            "output": output.decode("utf-8", errors="replace"),
            "error": error.decode("utf-8", errors="replace"),
        }
    finally:
        ACTIVE_CHILD = None


def settings():
    return {
        "email_provider": os.getenv("GENTS_EMAIL_PROVIDER", ""),
        "email_configured": bool(
            os.getenv("RESEND_API_KEY") or os.getenv("GENTS_EMAIL_PASSWORD")
        ),
        "email_from_configured": bool(os.getenv("GENTS_EMAIL_FROM")),
        "email_default_to_configured": bool(os.getenv("GENTS_EMAIL_DEFAULT_TO")),
        "email_allowed_recipient_count": len(
            allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS")
        ),
        "email_inbox_configured": bool(os.getenv("GENTS_EMAIL_INBOX")),
        "email_listener_enabled": os.getenv("GENTS_EMAIL_AUTO_START") == "1",
        "allowed_senders_configured": bool(os.getenv("GENTS_EMAIL_ALLOWED_SENDERS")),
        "hf_token_present": bool(os.getenv("HF_TOKEN")),
        "openai_codex_present": bool(os.getenv("RUST_OPENAI_API_KEY")),
        "openai_codex_model": os.getenv("GENTS_OPENAI_CODEX_MODEL", "gpt-5-codex"),
    }


def hf_headers():
    token = os.getenv("HF_TOKEN")
    if not token:
        raise ValueError(
            "Configure HF_TOKEN in .env.local or .env.company, then reload integrations"
        )
    return {"Authorization": "Bearer " + token}


def hf_dataset_search(query):
    headers = hf_headers()
    source = (
        "https://huggingface.co/api/datasets?limit=20&full=true&search="
        + urllib.parse.quote(query)
    )
    _, payload = web_request(source, headers=headers)
    rows = json.loads(payload)
    scope = "query"
    if not rows:
        # The operator's configured account is discoverable metadata. Keep the
        # complete whoami response (including email/auth details) out of output.
        _, identity_payload = web_request(
            "https://huggingface.co/api/whoami-v2", headers=headers
        )
        owner = json.loads(identity_payload).get("name", "")
        if not isinstance(owner, str) or not re.fullmatch(r"[\w.-]+", owner):
            raise ValueError(
                "Hugging Face account identity did not return a valid owner name"
            )
        source = (
            "https://huggingface.co/api/datasets?limit=100&full=true&author="
            + urllib.parse.quote(owner)
        )
        _, payload = web_request(source, headers=headers)
        rows = json.loads(payload)
        terms = re.findall(r"[a-z0-9]{3,}", query.lower())
        matching = [
            row
            for row in rows
            if any(term in row.get("id", "").lower() for term in terms)
        ]
        rows = matching or rows
        scope = "authenticated_account_after_empty_query"
    return {
        "query": query,
        "scope": scope,
        "source": source,
        "truncated": len(rows) > 20,
        "datasets": [
            {
                "id": row.get("id"),
                "sha": row.get("sha"),
                "private": row.get("private", False),
                "gated": row.get("gated", False),
                "last_modified": row.get("lastModified"),
            }
            for row in rows[:20]
            if row.get("id")
        ],
    }


def require_discovered_dataset(run_root, repo):
    searches = []
    for path in run_root.glob("tool-*.json"):
        if path.is_symlink():
            raise ValueError("journal symlinks forbidden")
        record = json.loads(path.read_text())
        if record.get("request", {}).get("action") != "hf_dataset_search":
            continue
        result = record.get("result", {}).get("result", {})
        if result.get("ok"):
            searches.append(result.get("result", {}).get("datasets", []))
    if searches and not any(row.get("id") == repo for rows in searches for row in rows):
        raise ValueError(
            "Dataset repo was not returned by this run's successful discovery. Use an actual returned owner/name; an empty query result requires broader or authenticated-account discovery, not a guessed repo or run UUID."
        )


def hf_dataset_metadata(repo, revision="main"):
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("dataset repo must be owner/name")
    if not isinstance(revision, str) or not revision or len(revision) > 200:
        raise ValueError("invalid dataset revision")
    encoded_repo = "/".join(
        urllib.parse.quote(part, safe="") for part in repo.split("/")
    )
    encoded_revision = urllib.parse.quote(revision, safe="")
    _, payload = web_request(
        f"https://huggingface.co/api/datasets/{encoded_repo}/revision/{encoded_revision}?blobs=true",
        headers=hf_headers(),
    )
    return json.loads(payload)


def safe_dataset_pdfs(metadata):
    records = []
    for entry in metadata.get("siblings", []):
        name = entry.get("rfilename", "")
        size = entry.get("size", entry.get("lfs", {}).get("size"))
        if (
            isinstance(name, str)
            and name.lower().endswith(".pdf")
            and isinstance(size, int)
        ):
            records.append({"path": name, "bytes": size})
    return records


def inspected_dataset_files(run_root, repo, revision, query="", offset=0, limit=10):
    if (
        not isinstance(query, str)
        or len(query) > 200
        or not isinstance(offset, int)
        or offset < 0
        or not isinstance(limit, int)
        or not 1 <= limit <= 20
    ):
        raise ValueError(
            "inventory query requires bounded text, offset >= 0 and limit 1..20"
        )
    for path in sorted(run_root.glob("tool-*.json"), reverse=True):
        if path.is_symlink():
            raise ValueError("journal symlinks forbidden")
        record = json.loads(path.read_text()).get("result", {})
        result = record.get("result", {})
        data = result.get("result", {})
        if (
            record.get("action") != "hf_dataset_inspect"
            or result.get("ok") is not True
            or data.get("repo") != repo
            or data.get("revision") != revision
        ):
            continue
        normalized_query = "".join(
            character for character in query.casefold() if character.isalnum()
        )

        def matches_query(row, normalized_query=normalized_query):
            path = row["path"].casefold()
            return (
                query.casefold() in path
                or bool(normalized_query)
                and normalized_query
                in "".join(character for character in path if character.isalnum())
            )

        matches = [row for row in data.get("pdfs", []) if matches_query(row)]
        rows = matches[offset : offset + limit]
        return {
            "repo": repo,
            "revision": revision,
            "pdfs": rows,
            "query": query,
            "offset": offset,
            "total_matches": len(matches),
            "inspection_step": record["step"],
            "match_mode": "literal_or_punctuation_normalized_path",
            "next_offset": offset + limit if offset + limit < len(matches) else None,
        }
    raise ValueError(
        "No successful inventory for this exact repo/revision. Inspect a discovered source first."
    )


def allowed_addresses(key):
    return {a.strip().lower() for a in os.getenv(key, "").split(",") if a.strip()}


def resend(path, method="GET", data=None, idempotency_key=None):
    token = os.getenv("RESEND_API_KEY")
    if not token:
        raise ValueError(
            "Configure RESEND_API_KEY and GENTS_EMAIL_FROM in .env.local or .env.company"
        )
    headers = {"Authorization": "Bearer " + token, "Content-Type": "application/json"}
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    return json.loads(
        web_request(
            "https://api.resend.com" + path,
            method,
            None if data is None else json.dumps(data).encode(),
            headers,
        )[1]
    )


def mailbox(args):
    senders = allowed_addresses("GENTS_EMAIL_ALLOWED_SENDERS")
    if not senders:
        raise ValueError(
            "Set GENTS_EMAIL_ALLOWED_SENDERS to trusted request senders before reading email"
        )
    if os.getenv("GENTS_EMAIL_PROVIDER") == "resend":
        inbox = os.getenv("GENTS_EMAIL_INBOX", "").strip().lower()
        if not re.fullmatch(r"[^\s@,<>]+@[^\s@,<>]+", inbox):
            raise ValueError(
                "Set GENTS_EMAIL_INBOX to the exact dedicated Resend receiving address; sending access does not grant Gmail inbox access"
            )

        def permitted(message):
            recipients = message.get("to", [])
            if isinstance(recipients, str):
                recipients = [recipients]
            return email_utils.parseaddr(message.get("from", ""))[
                1
            ].lower() in senders and inbox in {
                email_utils.parseaddr(address)[1].lower() for address in recipients
            }

        if args.get("id"):
            message = resend(
                "/emails/receiving/" + urllib.parse.quote(str(args["id"]), safe="")
            )
            if not permitted(message):
                raise ValueError(
                    "message is outside the configured sender and inbox policy"
                )
            return dict(message, untrusted_content=True)
        records = resend("/emails/receiving?limit=20").get("data", [])
        return {
            "messages": [m for m in records if permitted(m)],
            "untrusted_content": True,
        }
    host = os.getenv("GENTS_IMAP_HOST")
    if not host or not os.getenv("GENTS_EMAIL_PASSWORD"):
        raise ValueError(
            "Configure GENTS_IMAP_HOST, GENTS_EMAIL_USER, and GENTS_EMAIL_PASSWORD in .env.company"
        )
    with imaplib.IMAP4_SSL(host, ssl_context=tls_context()) as client:
        client.login(os.environ["GENTS_EMAIL_USER"], os.environ["GENTS_EMAIL_PASSWORD"])
        client.select(os.getenv("GENTS_IMAP_FOLDER", "INBOX"), readonly=True)
        status, data = client.uid("search", "ALL")
        if status != "OK":
            raise ValueError("mailbox search failed")
        ids = [str(args["id"]).encode()] if args.get("id") else data[0].split()[-20:]
        messages = []
        for uid in reversed(ids):
            if not uid.isdigit():
                raise ValueError("IMAP message id must be numeric")
            status, parts = client.uid("fetch", uid.decode(), "(BODY.PEEK[])")
            if status != "OK":
                continue
            raw = next((p[1] for p in parts if isinstance(p, tuple)), b"")
            if len(raw) > 2_000_000:
                continue
            item = email.message_from_bytes(raw, policy=default)
            sender = email_utils.parseaddr(str(item.get("From", "")))[1].lower()
            if sender not in senders:
                continue
            body = (
                item.get_body(preferencelist=("plain",))
                if item.is_multipart()
                else item
            )
            messages.append(
                {
                    "id": uid.decode(),
                    "from": sender,
                    "subject": str(item.get("Subject", "")),
                    "text": (body.get_content() if body else "")[:20000],
                    "message_id": str(item.get("Message-ID", "")),
                }
            )
        return {"messages": messages, "untrusted_content": True}


def workspace_sandbox(work, read_files=()):
    if sys.platform != "darwin" or not Path("/usr/bin/sandbox-exec").exists():
        raise ValueError("isolated execution requires macOS sandbox-exec on this host")
    temp = work / ".tmp"
    temp.mkdir(exist_ok=True)
    quoted = lambda path: json.dumps(str(path))
    allowed = [
        "/System",
        "/usr",
        "/bin",
        "/sbin",
        "/Library",
        "/private/etc",
        "/private/var/db",
        "/dev",
        "/opt/homebrew",
        work,
        ROOT / "node_modules",
        ROOT / ".venv-inference",
    ]
    document_python = ROOT / ".venv-inference/bin/python"
    if document_python.is_file():
        allowed.append(document_python.resolve().parents[1])
    node = shutil.which("node")
    if node:
        allowed.append(Path(node).resolve().parents[1])
    for extra in (Path.home() / ".rustup/toolchains", Path.home() / ".cargo/bin"):
        if extra.is_dir():
            allowed.append(extra)
    rules = [f"(subpath {quoted(path)})" for path in allowed]
    for path in read_files:
        if path.is_symlink() or not path.is_file():
            raise ValueError("trusted helper must be an existing non-symlink file")
        rules.append(f"(literal {quoted(path)})")
    reads = " ".join(rules)
    profile = "(version 1)(allow default)(deny network*)"
    profile += f'(deny file-read-data (require-all (require-any (subpath "/Users") (subpath "/Volumes") (subpath "/private/var/folders") (subpath "/private/tmp") (subpath "/private/var/tmp")) (require-not (require-any {reads}))))'
    profile += f'(deny file-write* (require-not (require-any (subpath {quoted(work)}) (literal "/dev/null"))))'
    profile += "(deny signal (require-not (target same-sandbox)))"
    env = {
        "PATH": str(ROOT / ".venv-inference/bin")
        + os.pathsep
        + os.getenv("PATH", "/usr/bin:/bin"),
        "HOME": str(work),
        "TMPDIR": str(temp),
        "CARGO_HOME": str(work / ".cargo"),
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
    }
    return profile, env


def approved_skills():
    base = ROOT / "docs/companies/we-need-getns-llc/shared-skills"
    catalog = base / "catalog.json"
    if not catalog.is_file() or catalog.is_symlink():
        return {}
    return {
        item["id"]: item
        for item in json.loads(catalog.read_text())["skills"]
        if item.get("status") in {"approved-demo", "approved-local"}
    }


def execute(company, run, action, args):
    if not re.fullmatch(r"[a-z0-9-]{1,80}", company) or not re.fullmatch(
        r"[a-f0-9-]{36}", run
    ):
        raise ValueError("invalid scope")
    work = confined(ROOT, f"data/companies/{company}/runs/{run}/workspace")
    work.mkdir(parents=True, exist_ok=True)
    skills = confined(work, ".skills")
    if action == "models_inventory":
        if args:
            raise ValueError("models_inventory accepts no arguments")
        from model_inventory import inventory

        return inventory(root=ROOT)
    if action == "integer_arithmetic":
        calculations = args.get("calculations")
        if (
            set(args) != {"calculations"}
            or not isinstance(calculations, list)
            or not 1 <= len(calculations) <= 12
        ):
            raise ValueError("provide 1..12 exact integer calculations")
        rows, seen = [], set()
        valid = lambda value: type(value) is int and abs(value) <= 10_000_000
        for item in calculations:
            if not isinstance(item, dict) or set(item) != {"id", "values", "subtract"}:
                raise ValueError("calculation requires id, values and subtract only")
            identity, values, subtract = item["id"], item["values"], item["subtract"]
            if (
                not isinstance(identity, str)
                or not re.fullmatch(r"[A-Za-z0-9-]{1,40}", identity)
                or identity in seen
            ):
                raise ValueError("invalid or duplicate calculation ID")
            if (
                not isinstance(values, list)
                or len(values) > 12
                or not all(valid(value) for value in values)
                or not valid(subtract)
            ):
                raise ValueError(
                    "only bounded signed integers are permitted; no floats or booleans"
                )
            seen.add(identity)
            total = sum(values)
            rows.append({"id": identity, "sum": total, "difference": total - subtract})
        return {"calculations": rows, "exact": True}
    if action == "source_read":
        target = source_target(args.get("path", ""))
        if not target.is_file() or target.stat().st_size > 1_000_000:
            raise ValueError("source file is missing or exceeds 1 MB")
        content = target.read_text(errors="replace")
        return {
            "path": str(target.relative_to(ROOT)),
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
            "content": content[:96000],
            "truncated": len(content) > 96000,
        }
    if action == "source_patch":
        target = source_target(args.get("path", ""))
        expected = args.get("expected_sha256", "")
        content = args.get("content")
        if not target.is_file() or not re.fullmatch(r"[a-f0-9]{64}", expected):
            raise ValueError("source patch requires an existing file and exact SHA-256")
        if not isinstance(content, str) or len(content.encode()) > 1_000_000:
            raise ValueError("replacement source content must be text up to 1 MB")
        original = target.read_bytes()
        actual = hashlib.sha256(original).hexdigest()
        if actual != expected:
            raise ValueError(
                "source changed after inspection; read it again before patching"
            )
        backup = confined(work, f".source-backups/{actual}-{target.name}")
        backup.parent.mkdir(exist_ok=True)
        backup.write_bytes(original)
        staged = target.with_name(target.name + f".gents-{run}.tmp")
        staged.write_text(content)
        staged.replace(target)
        try:
            checks = source_checks(target, work)
        except Exception:
            target.write_bytes(original)
            raise
        if checks["exit_code"] != 0:
            target.write_bytes(original)
            return {
                "ok": False,
                "rolled_back": True,
                "path": str(target.relative_to(ROOT)),
                "backup": str(backup.relative_to(work)),
                "checks": checks,
            }
        return {
            "ok": True,
            "rolled_back": False,
            "path": str(target.relative_to(ROOT)),
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
            "backup": str(backup.relative_to(work)),
            "checks": checks,
            "restart_required": True,
        }
    if action == "skill_compose":
        skill_id = args.get("id", "")
        roles = args.get("roles")
        steps = args.get("steps")
        purpose = args.get("purpose", "").strip()
        instructions = args.get("instructions", "").strip()
        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+){0,7}", skill_id):
            raise ValueError("skill id must be lowercase words separated by hyphens")
        if skill_id in approved_skills():
            raise ValueError(
                "an approved skill cannot be shadowed by a run composition"
            )
        if (
            not isinstance(roles, list)
            or not roles
            or not set(roles) <= {"planner", "worker"}
        ):
            raise ValueError("skill roles must contain planner and/or worker")
        if (
            not isinstance(steps, list)
            or not 1 <= len(steps) <= 8
            or any(
                step not in TOOLS
                or step in {"skill_compose", "skill_list", "skill_read"}
                for step in steps
            )
        ):
            raise ValueError(
                "skill steps must contain 1..8 existing non-registry tool ids"
            )
        if (
            not purpose
            or len(purpose) > 500
            or not instructions
            or len(instructions) > 8000
        ):
            raise ValueError("skill purpose/instructions are required and bounded")
        skills.mkdir(exist_ok=True)
        manifest = {
            "id": skill_id,
            "version": 1,
            "status": "approved-declarative",
            "roles": sorted(set(roles)),
            "steps": steps,
            "purpose": purpose,
            "instructions": instructions,
            "authority": "composition only; every requested tool step requires independent admin review",
        }
        target = confined(skills, skill_id + ".json")
        target.write_text(json.dumps(manifest, indent=2) + "\n")
        return {"ok": True, "skill": manifest, "path": str(target.relative_to(work))}
    if action == "skill_list":
        approved = [
            {key: value for key, value in item.items() if key != "path"}
            for item in approved_skills().values()
        ]
        if not skills.exists():
            return {"skills": [], "approved_skills": approved}
        return {
            "skills": [
                json.loads(path.read_text())
                for path in sorted(skills.glob("*.json"))[:100]
            ],
            "approved_skills": approved,
        }
    if action == "skill_read":
        skill_id = args.get("id", "")
        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+){0,7}", skill_id):
            raise ValueError("invalid skill id")
        entry = approved_skills().get(skill_id)
        if entry:
            target = confined(
                ROOT / "docs/companies/we-need-getns-llc/shared-skills", entry["path"]
            )
            if not target.is_file() or target.stat().st_size > 24000:
                raise ValueError(
                    "approved skill instructions missing or exceed the retrieval budget"
                )
            return {
                "skill": {
                    "id": skill_id,
                    "version": entry["version"],
                    "roles": entry["roles"],
                    "instructions": target.read_text(),
                    "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
                    "authority": "procedure only; every tool action still requires independent admin review",
                }
            }
        return {"skill": json.loads(confined(skills, skill_id + ".json").read_text())}
    if action == "journal_read":
        step = int(args["step"])
        if not 0 <= step <= 100000:
            raise ValueError("invalid journal step")
        journal = confined(work.parent, f"tool-{step:06}.json")
        record = json.loads(journal.read_text())
        if record.get("request", {}).get("action") == "journal_read":
            raise ValueError(
                "journal_read cannot read another journal_read result; select the original non-journal tool step from verified_progress"
            )
        return record
    if action == "pdf_render":
        source = confined(work, args.get("source", ""))
        output = confined(work, args.get("output", ""))
        if not source.is_file() or source.suffix != ".json" or output.suffix != ".pdf":
            raise ValueError(
                "pdf_render requires an existing JSON specification and an output .pdf path"
            )
        helper = ROOT / "scripts/document_renderer.py"
        profile, env = workspace_sandbox(work, read_files=[helper])
        result = subprocess.run(
            [
                "/usr/bin/sandbox-exec",
                "-p",
                profile,
                str(ROOT / ".venv-inference/bin/python"),
                str(helper),
            ],
            check=False,
            input=json.dumps({"source": str(source), "output": str(output)}),
            cwd=work,
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )
        if result.returncode:
            try:
                detail = json.loads(result.stdout)["error"]
            except (ValueError, KeyError):
                detail = "local rendering process failed"
            raise ValueError("PDF authoring failed: " + str(detail)[:1000])
        rendered = json.loads(result.stdout)
        rendered.update(
            {
                "path": str(output.relative_to(work)),
                "source": str(source.relative_to(work)),
            }
        )
        return rendered
    if action == "pdf_inspect":
        target = confined(work, args.get("path", ""))
        if not target.is_file() or target.suffix.lower() != ".pdf":
            raise ValueError("pdf_inspect requires an existing workspace PDF")
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        destination = confined(work, ".inspection/" + digest[:16])
        request = {
            "path": str(target),
            "destination": str(destination),
            "page_start": args.get("page_start", 1),
            "page_count": args.get("page_count", 4),
        }
        profile, env = workspace_sandbox(
            work, read_files=[ROOT / "scripts/pdf_tools.py"]
        )
        result = subprocess.run(
            [
                "/usr/bin/sandbox-exec",
                "-p",
                profile,
                str(ROOT / ".venv-inference/bin/python"),
                str(ROOT / "scripts/pdf_tools.py"),
            ],
            check=False,
            input=json.dumps(request),
            capture_output=True,
            text=True,
            timeout=150,
            env=env,
        )
        if result.returncode:
            try:
                error = json.loads(result.stdout)["error"]
                detail = f"[{error['code']}]: {error['message']}"
            except (ValueError, KeyError, TypeError):
                detail = "[inspection_process_failed]: verify the PDF and local document dependencies"
            raise ValueError("PDF inspection failed " + detail)
        inspected = json.loads(result.stdout)
        for page in inspected["inspected_pages"]:
            page["render"] = str(Path(page["render"]).relative_to(work))
        return inspected
    if action == "image_review":
        from vision_tools import LlamaCppVisionBackend, observation

        invocation = LlamaCppVisionBackend().prepare(ROOT, work, args)
        profile, env = workspace_sandbox(work, read_files=invocation.read_files)
        start = time.monotonic()
        result = memory_command(
            ["/usr/bin/sandbox-exec", "-p", profile, *invocation.argv], work, env
        )
        if result["exit_code"] != 0:
            error = result["error"].lower()
            category = (
                "context_budget" if "context" in error else "model_load_or_inference"
            )
            raise ValueError(
                f"local vision failed: {category}; exit code {result['exit_code']}. Inspect model/resource configuration"
            )
        return observation(
            invocation, result["output"], round((time.monotonic() - start) * 1000)
        )
    if action == "files":
        if args.get("op") == "rename":
            raise ValueError("File rename requires the governed native MCP adapter; no legacy fallback")
        if "metadata_only" in args:
            raise ValueError("Metadata-only file reads require the governed native MCP adapter")
        if "expected_sha256" in args:
            raise ValueError(
                "hash-conditioned writes require governed native MCP; legacy adapter cannot enforce this precondition"
            )
        target = confined(work, args.get("path", ""))
        if args["op"] == "write":
            content = args["content"].encode()
            if len(content) > 1_000_000:
                raise ValueError("file exceeds 1 MB")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
            return {"path": str(target.relative_to(ROOT)), "bytes": len(content)}
        if args["op"] == "read":
            with target.open("rb") as stream:
                text = stream.read(96000).decode("utf-8", errors="replace")[:24000]
            return {"path": str(target.relative_to(work)), "text": text}
        if args["op"] == "list":
            return {
                "files": [
                    str(p.relative_to(work)) for p in sorted(target.glob("*"))[:200]
                ]
            }
        raise ValueError("unknown files operation")
    if action in ("web_read", "web_search", "models_search"):
        url = args.get("url", "")
        if action == "web_search":
            url = "https://html.duckduckgo.com/html/?q=" + urllib.parse.quote(
                args["query"]
            )
        if action == "models_search":
            url = (
                "https://huggingface.co/api/models?limit=10&search="
                + urllib.parse.quote(args["query"])
            )
        source, data = web_request(url)
        if action == "models_search":
            return public_research_view({"source": source, "models": json.loads(data)})
        parser = PageText()
        parser.feed(data.decode("utf-8", errors="replace"))
        if action == "web_search" and (
            not parser.links or any("challenge" in text.lower() for text in parser.text)
        ):
            source, rss = web_request(
                "https://www.bing.com/search?format=rss&q="
                + urllib.parse.quote(args["query"])
            )
            items = ET.fromstring(rss).findall("./channel/item")
            results = [
                {
                    "title": item.findtext("title", ""),
                    "url": item.findtext("link", ""),
                    "snippet": item.findtext("description", ""),
                }
                for item in items[:10]
            ]
            results = relevant_search_results(args["query"], results)
            return public_research_view(
                {
                    "source": source,
                    "query": args["query"],
                    "results": results,
                    "links": [r["url"] for r in results],
                    "verification": "Keyword relevance check only; discovery snippets are not verified facts. Open and validate the actual sources.",
                    "retrieved_at": int(time.time()),
                    "untrusted_content": True,
                }
            )
        return public_research_view(
            {
                "source": source,
                "retrieved_at": int(time.time()),
                "text": "\n".join(parser.text)[:24000],
                "links": list(dict.fromkeys(parser.links))[:30],
                "untrusted_content": True,
            }
        )
    if action == "browser":
        request = work / ".browser-request.json"
        request.write_text(json.dumps(args))
        return command(
            ["node", str(ROOT / "scripts/company-browser.mjs"), str(request)],
            work,
            timeout=180,
        )
    if action == "shell":
        profile, env = workspace_sandbox(work)
        return command(
            [
                "/usr/bin/sandbox-exec",
                "-p",
                profile,
                "/bin/zsh",
                "-f",
                "-c",
                args["command"],
            ],
            work,
            env,
            timeout=min(max(int(args.get("timeout_seconds", 300)), 1), 3600),
        )
    if action == "resources":
        memory = (
            subprocess.run(
                ["sysctl", "-n", "hw.memsize"],
                check=False,
                capture_output=True,
                text=True,
            ).stdout.strip()
            if sys.platform == "darwin"
            else "unknown"
        )
        rows = subprocess.run(
            ["ps", "-axo", "pid=,uid=,lstart=,rss=,comm="],
            check=False,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        processes = []
        for row in rows:
            fields = row.split(None, 8)
            if len(fields) == 9 and fields[1] == str(os.getuid()):
                processes.append(
                    {
                        "pid": int(fields[0]),
                        "identity": " ".join(fields[1:7]) + " " + fields[8],
                        "memory_kb": int(fields[7]),
                        "executable": fields[8],
                    }
                )
        return {
            "now_utc": dt.datetime.now(dt.UTC).isoformat(),
            "memory_bytes": memory,
            "disk_free_bytes": shutil.disk_usage(ROOT).free,
            "models": [
                {"path": str(p.relative_to(ROOT / "models")), "bytes": p.stat().st_size}
                for p in (ROOT / "models").rglob("*")
                if p.is_file() and p.suffix in (".gguf", ".safetensors")
            ],
            "processes": sorted(processes, key=lambda p: p["memory_kb"], reverse=True)[
                :30
            ],
            "settings": settings(),
            "document_tools": {
                "python": str(ROOT / ".venv-inference/bin/python"),
                "generation": "python3 with reportlab via shell",
                "inspection": "pdf_inspect",
                "pdftoppm_present": bool(shutil.which("pdftoppm")),
            },
        }
    if action == "hf_dataset_search":
        query = args.get("query", "").strip()
        if not query or len(query) > 200:
            raise ValueError("dataset search query requires 1..200 characters")
        return hf_dataset_search(query)
    if action == "hf_dataset_inspect":
        require_discovered_dataset(work.parent, args.get("repo", ""))
        metadata = hf_dataset_metadata(
            args.get("repo", ""), args.get("revision", "main")
        )
        return {
            "repo": metadata.get("id"),
            "revision": metadata.get("sha"),
            "private": metadata.get("private", False),
            "gated": metadata.get("gated", False),
            "pdfs": safe_dataset_pdfs(metadata),
        }
    if action == "hf_dataset_files":
        return inspected_dataset_files(
            work.parent,
            args.get("repo", ""),
            args.get("revision", ""),
            args.get("query", ""),
            args.get("offset", 0),
            args.get("limit", 10),
        )
    if action == "hf_dataset_download":
        repo, revision = args.get("repo", ""), args.get("revision", "")
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo) or not re.fullmatch(
            r"[a-f0-9]{40}", revision
        ):
            raise ValueError(
                "dataset download requires owner/name and an immutable 40-character commit"
            )
        require_discovered_dataset(work.parent, repo)
        files = args.get("files")
        if not isinstance(files, list) or not files or len(files) > 200:
            raise ValueError("select 1..200 exact PDF files")
        if any(
            not isinstance(name, str)
            or name.startswith("-")
            or not name.lower().endswith(".pdf")
            or Path(name).is_absolute()
            or any(part in (".", "..") for part in Path(name).parts)
            for name in files
        ):
            raise ValueError("dataset download accepts exact relative .pdf paths only")
        metadata = hf_dataset_metadata(repo, revision)
        available = {row["path"]: row["bytes"] for row in safe_dataset_pdfs(metadata)}
        if metadata.get("sha") != revision or any(
            name not in available for name in files
        ):
            raise ValueError("requested dataset revision/PDF was not found")
        if (
            sum(available[name] for name in files) + 5 * 1024**3
            > shutil.disk_usage(ROOT).free
        ):
            raise ValueError(
                "not enough disk for selected PDFs plus a 5 GiB working reserve"
            )
        destination = confined(work, args.get("destination", "huggingface-pdfs"))
        destination.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ, HF_HUB_OFFLINE="0", TRANSFORMERS_OFFLINE="0")
        result = command(
            [
                shutil.which("hf") or "hf",
                "download",
                repo,
                *files,
                "--type",
                "dataset",
                "--revision",
                revision,
                "--local-dir",
                str(destination),
                "--max-workers",
                "2",
                "--format",
                "json",
            ],
            work,
            env,
            timeout=24 * 3600,
        )
        if result["exit_code"] != 0:
            raise ValueError(
                "Hugging Face dataset download failed; verify repo access and selected files"
            )
        downloaded = []
        for name in files:
            target = confined(destination, name)
            if (
                not target.is_file()
                or target.is_symlink()
                or target.stat().st_size != available[name]
            ):
                raise ValueError("downloaded PDF failed path/size verification")
            digest = hashlib.sha256()
            with target.open("rb") as stream:
                for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
                    digest.update(block)
            downloaded.append(
                {
                    "path": str(target.relative_to(work)),
                    "bytes": target.stat().st_size,
                    "sha256": digest.hexdigest(),
                }
            )
        return {"repo": repo, "revision": revision, "files": downloaded}
    if action == "model_download":
        repo, revision = args["repo"], args["revision"]
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo) or not re.fullmatch(
            r"[a-f0-9]{40}", revision
        ):
            raise ValueError(
                "download requires owner/repo and an immutable 40-character commit from model metadata"
            )
        dest = confined(ROOT / "models", args["destination"])
        filenames = args["files"]
        if not isinstance(filenames, list) or not filenames or len(filenames) > 100:
            raise ValueError("select explicit model files")
        if any(
            not isinstance(f, str)
            or f.startswith("-")
            or Path(f).suffix
            not in (
                ".gguf",
                ".safetensors",
                ".json",
                ".model",
                ".txt",
                ".jinja",
                ".tiktoken",
            )
            for f in filenames
        ):
            raise ValueError(
                "only weights/tokenizer/config files may be downloaded; no executable code"
            )
        for filename in filenames:
            confined(dest, filename)
        metadata = json.loads(
            web_request(
                f"https://huggingface.co/api/models/{repo}/revision/{revision}?blobs=true"
            )[1]
        )
        available = {
            entry["rfilename"]: entry for entry in metadata.get("siblings", [])
        }
        if metadata.get("sha") != revision or any(
            f not in available for f in filenames
        ):
            raise ValueError("requested revision/file not found")
        sizes = [
            available[f].get("size", available[f].get("lfs", {}).get("size"))
            for f in filenames
        ]
        if any(size is None for size in sizes):
            raise ValueError(
                "source metadata has no size; inspect model before downloading"
            )
        if sum(sizes) + 5 * 1024**3 > shutil.disk_usage(ROOT).free:
            raise ValueError(
                "not enough disk for download plus a 5 GiB working reserve; choose a smaller quantization"
            )
        provenance = confined(dest, "gents-download.json")
        if any(confined(dest, filename).exists() for filename in filenames):
            if not provenance.is_file():
                raise ValueError(
                    "destination already contains model files without matching provenance; choose a new folder"
                )
            previous = json.loads(provenance.read_text())
            if previous.get("repo") != repo or previous.get("revision") != revision:
                raise ValueError(
                    "download cannot overwrite a different model revision; choose a new folder"
                )
            for filename in filenames:
                target = confined(dest, filename)
                if target.exists():
                    digest = hashlib.sha256()
                    with target.open("rb") as stream:
                        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
                            digest.update(block)
                    if digest.hexdigest() != previous.get("sha256", {}).get(filename):
                        raise ValueError(
                            "existing model file changed; preserve it and choose a new download folder"
                        )
        dest.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ, HF_HUB_OFFLINE="0", TRANSFORMERS_OFFLINE="0")
        result = command(
            [
                shutil.which("hf") or "hf",
                "download",
                repo,
                *filenames,
                "--revision",
                revision,
                "--local-dir",
                str(dest),
                "--max-workers",
                "1",
            ],
            work,
            env,
            timeout=24 * 3600,
        )
        if result["exit_code"] != 0:
            return {
                "exit_code": result["exit_code"],
                "error": "Model download failed; check access/license or network. No token was logged.",
            }
        hashes = {}
        for filename in filenames:
            path = confined(dest, filename)
            if path.stat().st_size != available[filename].get(
                "size", available[filename].get("lfs", {}).get("size")
            ):
                raise ValueError("downloaded model size mismatch")
            hasher = hashlib.sha256()
            with path.open("rb") as stream:
                for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
                    hasher.update(block)
            hashes[filename] = hasher.hexdigest()
            expected = available[filename].get("lfs", {}).get("sha256")
            if expected and hashes[filename] != expected:
                raise ValueError("downloaded model hash mismatch")
        provenance.write_text(
            json.dumps({"repo": repo, "revision": revision, "sha256": hashes}, indent=2)
        )
        return {
            "destination": str(dest.relative_to(ROOT)),
            "revision": revision,
            "sha256": hashes,
        }
    if action == "model_quantize":
        source = confined(ROOT / "models", args["source"])
        dest = confined(ROOT / "models", args["destination"])
        if args.get("format") == "mlx":
            if (
                not source.is_dir()
                or dest.exists()
                or int(args.get("bits", 4)) not in (2, 4, 8)
            ):
                raise ValueError(
                    "MLX quantization requires a local model directory, a new destination, and 2/4/8 bits"
                )
            total = sum(p.stat().st_size for p in source.rglob("*.safetensors"))
            if total == 0 or shutil.disk_usage(ROOT).free < total + 5 * 1024**3:
                raise ValueError(
                    "missing Safetensors or insufficient disk reserve for conversion"
                )
            return command(
                [
                    str(ROOT / ".venv-inference/bin/python"),
                    "-m",
                    "mlx_lm",
                    "convert",
                    "--hf-path",
                    str(source),
                    "--mlx-path",
                    str(dest),
                    "-q",
                    "--q-bits",
                    str(int(args.get("bits", 4))),
                ],
                work,
                dict(os.environ, HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1"),
                timeout=24 * 3600,
            )
        quant = args.get("type", "Q4_K_M")
        if (
            quant not in ("Q4_K_M", "Q5_K_M", "Q3_K_M", "Q2_K")
            or source.suffix != ".gguf"
            or dest.suffix != ".gguf"
            or dest.exists()
        ):
            raise ValueError(
                "choose a supported quantization and a NEW GGUF destination; originals are preserved"
            )
        if shutil.disk_usage(ROOT).free < source.stat().st_size + 5 * 1024**3:
            raise ValueError("insufficient disk to quantize while retaining original")
        dest.parent.mkdir(parents=True, exist_ok=True)
        return command(
            [
                shutil.which("llama-quantize") or "llama-quantize",
                str(source),
                str(dest),
                quant,
            ],
            work,
            timeout=24 * 3600,
        )
    if action == "email_list":
        return mailbox(args)
    if action == "schedule":
        import company_schedule

        return company_schedule.operation(company, run, args)
    if action == "email_status":
        if os.getenv("GENTS_EMAIL_PROVIDER") != "resend" or not re.fullmatch(
            r"[a-f0-9-]{36}", args.get("id", "")
        ):
            raise ValueError("email_status needs a Resend email UUID")
        message = resend("/emails/" + args["id"])
        if (
            not message.get("to")
            or not {a.strip().lower() for a in message["to"]}.issubset(
                allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS")
            )
            or email_utils.parseaddr(message.get("from", ""))[1].lower()
            != email_utils.parseaddr(os.getenv("GENTS_EMAIL_FROM", ""))[1].lower()
        ):
            raise ValueError("email is outside the configured sender/recipient policy")
        return {
            key: message.get(key)
            for key in ("id", "to", "from", "subject", "last_event", "scheduled_at")
        }
    if action == "email_send":
        recipient = args["to"].strip().lower()
        if recipient not in allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS"):
            raise ValueError("recipient must be in GENTS_EMAIL_ALLOWED_RECIPIENTS")
        sender = os.environ["GENTS_EMAIL_FROM"]
        payload = {
            "from": sender,
            "to": [recipient],
            "subject": args["subject"],
            "text": args["text"],
        }
        if args.get("scheduled_at"):
            when = dt.datetime.fromisoformat(args["scheduled_at"])
            if when.tzinfo is None or not dt.timedelta(0) < when - dt.datetime.now(
                dt.UTC
            ) <= dt.timedelta(days=30):
                raise ValueError(
                    "scheduled_at must be an explicit ISO8601 timestamp with offset, in the next 30 days; never silently send a missed schedule immediately"
                )
            if os.getenv("GENTS_EMAIL_PROVIDER") != "resend":
                raise ValueError("scheduled delivery requires Resend")
            payload["scheduled_at"] = when.isoformat()
        if os.getenv("GENTS_EMAIL_PROVIDER") == "resend":
            return resend("/emails", "POST", payload, args["idempotency_key"])
        message = EmailMessage()
        message["From"], message["To"], message["Subject"] = (
            sender,
            recipient,
            args["subject"],
        )
        message["Message-ID"] = f"<{args['idempotency_key']}@gents.local>"
        message.set_content(args["text"])
        with smtplib.SMTP_SSL(
            os.environ["GENTS_SMTP_HOST"],
            int(os.getenv("GENTS_SMTP_PORT", "465")),
            context=tls_context(),
        ) as client:
            client.login(
                os.environ["GENTS_EMAIL_USER"], os.environ["GENTS_EMAIL_PASSWORD"]
            )
            client.send_message(message)
        return {"sent": True, "to": recipient, "message_id": message["Message-ID"]}
    if action == "training_prepare":
        from train_role import digest, samples

        training = __import__("uuid").uuid4().hex
        # UUIDs are used by the existing training admission API.
        training = str(__import__("uuid").UUID(training))
        dataset = confined(
            ROOT, f"data/companies/{company}/training/datasets/{training}"
        )
        dataset.mkdir(parents=True)
        for split in ("train", "valid", "test"):
            if args.get("dataset_path"):
                source = confined(work, args["dataset_path"]) / f"{split}.jsonl"
                if source.is_symlink() or source.stat().st_size > 10 * 1024 * 1024:
                    raise ValueError(
                        "reviewed dataset must be regular JSONL under 10 MiB"
                    )
                shutil.copyfile(source, dataset / f"{split}.jsonl")
            else:
                (dataset / f"{split}.jsonl").write_text(
                    "\n".join(json.dumps(row) for row in args["samples"][split]) + "\n"
                )
        rows = samples(dataset, company, args["role"])
        return {
            "manifest": {
                "company_id": company,
                "training_id": training,
                "role": args["role"],
                "base_model": args["base_model"],
                "dataset_sha256": {s: digest(dataset / f"{s}.jsonl") for s in rows},
                "dataset_counts": {s: len(rows[s]) for s in rows},
                "iterations": min(max(int(args.get("iterations", 100)), 1), 1000),
                "sequence_length": 512,
                "smoke": False,
                "reviewed_dataset": True,
            }
        }
    if action == "training_status":
        path = confined(
            ROOT, f"data/companies/{company}/training/runs/{args['training_id']}"
        )
        return {
            name: json.loads((path / name).read_text())
            for name in ("progress.json", "metrics.json")
            if (path / name).is_file()
        }
    if action == "process_stop":
        pid = int(args["pid"])
        if pid <= 1 or pid in (os.getpid(), os.getppid()):
            raise ValueError("cannot stop system or harness processes")
        identity = " ".join(
            subprocess.run(
                ["ps", "-p", str(pid), "-o", "uid=,lstart=,comm="],
                check=False,
                capture_output=True,
                text=True,
            ).stdout.split()
        )
        if (
            identity != " ".join(args["identity"].split())
            or not identity
            or identity.split()[0] != str(os.getuid())
        ):
            raise ValueError(
                "process identity changed or process belongs to another user"
            )
        if args.get("operator_confirmed") is not True:
            raise ValueError(
                "non-company processes require exact operator confirmation"
            )
        os.kill(pid, signal.SIGTERM)
        return {"termination_requested": pid, "identity": identity}
    raise ValueError("unsupported tool: " + action)


if __name__ == "__main__":

    def stop_owned(_signal, _frame):
        if ACTIVE_CHILD and ACTIVE_CHILD.poll() is None:
            os.killpg(ACTIVE_CHILD.pid, signal.SIGTERM)
        raise SystemExit(143)

    signal.signal(signal.SIGTERM, stop_owned)
    signal.signal(signal.SIGINT, stop_owned)
    parser = argparse.ArgumentParser()
    parser.add_argument("--company", required=True)
    parser.add_argument("--run", required=True)
    args = parser.parse_args()
    try:
        request = json.load(sys.stdin)
        result = execute(
            args.company, args.run, request["action"], request["arguments"]
        )
        print(json.dumps({"ok": True, "result": result}, ensure_ascii=True))
    except (
        ValueError,
        KeyError,
        OSError,
        http.client.HTTPException,
        smtplib.SMTPException,
        imaplib.IMAP4.error,
        subprocess.SubprocessError,
    ) as error:
        # Controlled errors omit credentialed HTTP bodies and SMTP/IMAP auth details.
        detail = (
            str(error)
            if isinstance(error, (ValueError, KeyError, FileNotFoundError))
            else type(error).__name__
        )
        print(json.dumps({"ok": False, "error": detail}))
