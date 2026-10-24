#!/usr/bin/env python3
"""Typed company tools. Called only after a recorded admin-judge decision.

Prompts/web/email are untrusted data. Secrets never enter model context. Shells
run in a per-run macOS sandbox; model downloads never execute repository code.
"""
import argparse
import datetime as dt
import email
from email.message import EmailMessage
from email.policy import default
import hashlib
import http.client
from html.parser import HTMLParser
import imaplib
import ipaddress
import json
import os
from pathlib import Path
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

ROOT = Path(__file__).resolve().parents[1]
ACTIVE_CHILD = None
TOOLS = {
    "web_search": "query: public search string",
    "web_read": "url: public HTTPS page; returns source URL and extracted text",
    "browser": "url; actions: [{kind:click|fill, selector, value?}]; screenshot captured",
    "files": "op: list|read|write; path: relative to run workspace; content for write",
    "shell": "command: run code/tests in isolated run workspace; timeout_seconds up to 3600; no network or home access",
    "resources": "Inspect free disk, system RAM, local model files, and process resource usage",
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


def public_address(host):
    addresses = socket.getaddrinfo(host, 443, type=socket.SOCK_STREAM)
    if not addresses or any(not ipaddress.ip_address(item[4][0]).is_global for item in addresses):
        raise ValueError("web tools require public internet hosts, not local/private addresses")
    return addresses[0][4][0]


def tls_context():
    context = ssl.create_default_context()
    if Path("/etc/ssl/cert.pem").is_file():
        context.load_verify_locations("/etc/ssl/cert.pem")
    return context


class PinnedHTTPS(http.client.HTTPSConnection):
    def connect(self):
        # Pin the validated address to prevent DNS rebinding between validation/connect.
        sock = socket.create_connection((public_address(self.host), self.port), self.timeout)
        self.sock = self._context.wrap_socket(sock, server_hostname=self.host)


def web_request(url, method="GET", payload=None, headers=None, limit=2_000_000):
    for _ in range(6):
        parsed = urllib.parse.urlsplit(url)
        if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password or parsed.port not in (None, 443):
            raise ValueError("public HTTPS on port 443 required, without embedded credentials")
        connection = PinnedHTTPS(parsed.hostname, timeout=40, context=tls_context())
        try:
            connection.request(method, urllib.parse.urlunsplit(("", "", parsed.path or "/", parsed.query, "")), body=payload,
                               headers={"User-Agent": "GentsCompany/1.0", **(headers or {})})
            response = connection.getresponse()
            if response.status in (301, 302, 303, 307, 308):
                if headers and "Authorization" in headers:
                    raise ValueError("credentialed requests cannot redirect")
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
            if href.startswith("//duckduckgo.com/l/?"):
                href = urllib.parse.parse_qs(urllib.parse.urlsplit("https:" + href).query).get("uddg", [href])[0]
            if href.startswith("https://"):
                self.links.append(href)
    def handle_endtag(self, tag):
        if tag in ("script", "style", "noscript"):
            self.skip = max(0, self.skip - 1)
    def handle_data(self, data):
        if not self.skip and data.strip():
            self.text.append(data.strip())


def relevant_search_results(query, results):
    # Some public RSS search responses silently ignore the query and return
    # generic news homepages. Do not present those as topical research evidence.
    stop = set("the and for with from about latest new news current recent today breaking updates related anything companies company products product announcements".split())
    terms = {term.casefold() for term in re.findall(r"[\w-]+", query) if len(term) >= 2} - stop
    if "ai" in terms:
        terms.update(("artificial", "intelligence"))
    if not terms:
        raise ValueError("Search query is too broad; include a specific subject or source")
    matches = [row for row in results if any(re.search(r"\b" + re.escape(term) + r"\b", " ".join(row.values()), re.I) for term in terms)]
    if not matches:
        raise ValueError("Public search returned only off-topic results, not evidence for this query. Narrow the topic/source query or use web_read on known official sources. Do not invent findings from these results.")
    return matches


def command(args, cwd, env=None, timeout=300):
    global ACTIVE_CHILD
    # Disk-backed logs avoid unlimited output in memory; return a bounded tail.
    log = cwd / ".tool-output.log"
    with log.open("wb") as stream:
        process = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                   stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
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


def settings():
    return {"email_provider": os.getenv("GENTS_EMAIL_PROVIDER", ""),
            "email_configured": bool(os.getenv("RESEND_API_KEY") or os.getenv("GENTS_EMAIL_PASSWORD")),
            "email_from": os.getenv("GENTS_EMAIL_FROM", ""),
            "email_default_to": os.getenv("GENTS_EMAIL_DEFAULT_TO", ""),
            "email_allowed_recipients": sorted(allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS")),
            "email_inbox": os.getenv("GENTS_EMAIL_INBOX", ""),
            "email_listener_enabled": os.getenv("GENTS_EMAIL_AUTO_START") == "1",
            "allowed_senders_configured": bool(os.getenv("GENTS_EMAIL_ALLOWED_SENDERS")),
            "hf_token_present": bool(os.getenv("HF_TOKEN"))}


def allowed_addresses(key):
    return {a.strip().lower() for a in os.getenv(key, "").split(",") if a.strip()}


def resend(path, method="GET", data=None, idempotency_key=None):
    token = os.getenv("RESEND_API_KEY")
    if not token:
        raise ValueError("Configure RESEND_API_KEY and GENTS_EMAIL_FROM in .env.local or .env.company")
    headers = {"Authorization": "Bearer " + token, "Content-Type": "application/json"}
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    return json.loads(web_request("https://api.resend.com" + path, method,
                                 None if data is None else json.dumps(data).encode(), headers)[1])


def mailbox(args):
    senders = allowed_addresses("GENTS_EMAIL_ALLOWED_SENDERS")
    if not senders:
        raise ValueError("Set GENTS_EMAIL_ALLOWED_SENDERS to trusted request senders before reading email")
    if os.getenv("GENTS_EMAIL_PROVIDER") == "resend":
        inbox = os.getenv("GENTS_EMAIL_INBOX", "").strip().lower()
        if not re.fullmatch(r"[^\s@,<>]+@[^\s@,<>]+", inbox):
            raise ValueError("Set GENTS_EMAIL_INBOX to the exact dedicated Resend receiving address; sending access does not grant Gmail inbox access")

        def permitted(message):
            recipients = message.get("to", [])
            if isinstance(recipients, str):
                recipients = [recipients]
            return (email.utils.parseaddr(message.get("from", ""))[1].lower() in senders
                    and inbox in {email.utils.parseaddr(address)[1].lower() for address in recipients})

        if args.get("id"):
            message = resend("/emails/receiving/" + urllib.parse.quote(str(args["id"]), safe=""))
            if not permitted(message):
                raise ValueError("message is outside the configured sender and inbox policy")
            return dict(message, untrusted_content=True)
        records = resend("/emails/receiving?limit=20").get("data", [])
        return {"messages": [m for m in records if permitted(m)], "untrusted_content": True}
    host = os.getenv("GENTS_IMAP_HOST")
    if not host or not os.getenv("GENTS_EMAIL_PASSWORD"):
        raise ValueError("Configure GENTS_IMAP_HOST, GENTS_EMAIL_USER, and GENTS_EMAIL_PASSWORD in .env.company")
    with imaplib.IMAP4_SSL(host, ssl_context=tls_context()) as client:
        client.login(os.environ["GENTS_EMAIL_USER"], os.environ["GENTS_EMAIL_PASSWORD"])
        client.select(os.getenv("GENTS_IMAP_FOLDER", "INBOX"), readonly=True)
        status, data = client.uid("search", None, "ALL")
        if status != "OK":
            raise ValueError("mailbox search failed")
        ids = [str(args["id"]).encode()] if args.get("id") else data[0].split()[-20:]
        messages = []
        for uid in reversed(ids):
            if not uid.isdigit():
                raise ValueError("IMAP message id must be numeric")
            status, parts = client.uid("fetch", uid, "(BODY.PEEK[])")
            if status != "OK":
                continue
            raw = next((p[1] for p in parts if isinstance(p, tuple)), b"")
            if len(raw) > 2_000_000:
                continue
            item = email.message_from_bytes(raw, policy=default)
            sender = email.utils.parseaddr(str(item.get("From", "")))[1].lower()
            if sender not in senders:
                continue
            body = item.get_body(preferencelist=("plain",)) if item.is_multipart() else item
            messages.append({"id": uid.decode(), "from": sender, "subject": str(item.get("Subject", "")),
                             "text": (body.get_content() if body else "")[:20000],
                             "message_id": str(item.get("Message-ID", ""))})
        return {"messages": messages, "untrusted_content": True}


def execute(company, run, action, args):
    if not re.fullmatch(r"[a-z0-9-]{1,80}", company) or not re.fullmatch(r"[a-f0-9-]{36}", run):
        raise ValueError("invalid scope")
    work = confined(ROOT, f"data/companies/{company}/runs/{run}/workspace")
    work.mkdir(parents=True, exist_ok=True)
    if action == "journal_read":
        step = int(args["step"])
        if not 0 <= step <= 100000:
            raise ValueError("invalid journal step")
        journal = confined(work.parent, f"tool-{step:06}.json")
        return json.loads(journal.read_text())
    if action == "files":
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
            return {"files": [str(p.relative_to(work)) for p in sorted(target.glob("*"))[:200]]}
        raise ValueError("unknown files operation")
    if action in ("web_read", "web_search", "models_search"):
        url = args.get("url", "")
        if action == "web_search":
            url = "https://html.duckduckgo.com/html/?q=" + urllib.parse.quote(args["query"])
        if action == "models_search":
            url = "https://huggingface.co/api/models?limit=10&search=" + urllib.parse.quote(args["query"])
        source, data = web_request(url)
        if action == "models_search":
            return {"source": source, "models": json.loads(data)}
        parser = PageText()
        parser.feed(data.decode("utf-8", errors="replace"))
        if action == "web_search" and (not parser.links or any("challenge" in text.lower() for text in parser.text)):
            source, rss = web_request("https://www.bing.com/search?format=rss&q=" + urllib.parse.quote(args["query"]))
            items = ET.fromstring(rss).findall("./channel/item")
            results = [{"title": item.findtext("title", ""), "url": item.findtext("link", ""),
                        "snippet": item.findtext("description", "")} for item in items[:10]]
            results = relevant_search_results(args["query"], results)
            return {"source": source, "query": args["query"], "results": results, "links": [r["url"] for r in results],
                    "verification": "Keyword relevance check only; discovery snippets are not verified facts. Open and validate the actual sources.",
                    "retrieved_at": int(time.time()), "untrusted_content": True}
        return {"source": source, "retrieved_at": int(time.time()), "text": "\n".join(parser.text)[:24000],
                "links": list(dict.fromkeys(parser.links))[:30], "untrusted_content": True}
    if action == "browser":
        request = work / ".browser-request.json"
        request.write_text(json.dumps(args))
        return command(["node", str(ROOT / "scripts/company-browser.mjs"), str(request)], work, timeout=180)
    if action == "shell":
        if sys.platform != "darwin" or not Path("/usr/bin/sandbox-exec").exists():
            raise ValueError("isolated shell requires macOS sandbox-exec on this host")
        temp = work / ".tmp"
        temp.mkdir(exist_ok=True)
        quoted = lambda p: json.dumps(str(p))
        allowed = ["/System", "/usr", "/bin", "/sbin", "/Library", "/private/etc", "/private/var/db", "/dev", "/opt/homebrew", work, ROOT / "node_modules", ROOT / ".venv-inference"]
        node = shutil.which("node")
        if node:
            allowed.append(Path(node).resolve().parents[1])
        for extra in (Path.home() / ".rustup/toolchains", Path.home() / ".cargo/bin"):
            if extra.is_dir():
                allowed.append(extra)
        rules = ' '.join(f'(subpath {quoted(p)})' for p in allowed)
        # Keep metadata/system IPC available for macOS dyld while restricting
        # actual user-file contents, writes, network, and signals to host apps.
        profile = '(version 1)(allow default)(deny network*)'
        profile += f'(deny file-read-data (require-all (require-any (subpath "/Users") (subpath "/Volumes") (subpath "/private/var/folders") (subpath "/private/tmp") (subpath "/private/var/tmp")) (require-not (require-any {rules}))))'
        profile += f'(deny file-write* (require-not (require-any (subpath {quoted(work)}) (literal "/dev/null"))))'
        profile += '(deny signal (require-not (target same-sandbox)))'
        env = {"PATH": os.getenv("PATH", "/usr/bin:/bin"), "HOME": str(work), "TMPDIR": str(temp),
               "CARGO_HOME": str(work / ".cargo"), "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"}
        return command(["/usr/bin/sandbox-exec", "-p", profile, "/bin/zsh", "-f", "-c", args["command"]], work, env,
                       timeout=min(max(int(args.get("timeout_seconds", 300)), 1), 3600))
    if action == "resources":
        memory = subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True).stdout.strip() if sys.platform == "darwin" else "unknown"
        rows = subprocess.run(["ps", "-axo", "pid=,uid=,lstart=,rss=,comm="], capture_output=True, text=True).stdout.splitlines()
        processes = []
        for row in rows:
            fields = row.split(None, 8)
            if len(fields) == 9 and fields[1] == str(os.getuid()):
                processes.append({"pid": int(fields[0]), "identity": " ".join(fields[1:7]) + " " + fields[8],
                                  "memory_kb": int(fields[7]), "executable": fields[8]})
        return {"now_utc": dt.datetime.now(dt.timezone.utc).isoformat(), "memory_bytes": memory, "disk_free_bytes": shutil.disk_usage(ROOT).free,
                "models": [{"path": str(p.relative_to(ROOT / "models")), "bytes": p.stat().st_size} for p in (ROOT / "models").rglob("*") if p.is_file() and p.suffix in (".gguf", ".safetensors")],
                "processes": sorted(processes, key=lambda p: p["memory_kb"], reverse=True)[:30], "settings": settings()}
    if action == "model_download":
        repo, revision = args["repo"], args["revision"]
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo) or not re.fullmatch(r"[a-f0-9]{40}", revision):
            raise ValueError("download requires owner/repo and an immutable 40-character commit from model metadata")
        dest = confined(ROOT / "models", args["destination"])
        filenames = args["files"]
        if not isinstance(filenames, list) or not filenames or len(filenames) > 100:
            raise ValueError("select explicit model files")
        if any(not isinstance(f, str) or f.startswith("-") or Path(f).suffix not in (".gguf", ".safetensors", ".json", ".model", ".txt", ".jinja", ".tiktoken") for f in filenames):
            raise ValueError("only weights/tokenizer/config files may be downloaded; no executable code")
        for filename in filenames:
            confined(dest, filename)
        metadata = json.loads(web_request(f"https://huggingface.co/api/models/{repo}/revision/{revision}?blobs=true")[1])
        available = {entry["rfilename"]: entry for entry in metadata.get("siblings", [])}
        if metadata.get("sha") != revision or any(f not in available for f in filenames):
            raise ValueError("requested revision/file not found")
        sizes = [available[f].get("size", available[f].get("lfs", {}).get("size")) for f in filenames]
        if any(size is None for size in sizes):
            raise ValueError("source metadata has no size; inspect model before downloading")
        if sum(sizes) + 5 * 1024**3 > shutil.disk_usage(ROOT).free:
            raise ValueError("not enough disk for download plus a 5 GiB working reserve; choose a smaller quantization")
        provenance = confined(dest, "gents-download.json")
        if any(confined(dest, filename).exists() for filename in filenames):
            if not provenance.is_file():
                raise ValueError("destination already contains model files without matching provenance; choose a new folder")
            previous = json.loads(provenance.read_text())
            if previous.get("repo") != repo or previous.get("revision") != revision:
                raise ValueError("download cannot overwrite a different model revision; choose a new folder")
            for filename in filenames:
                target = confined(dest, filename)
                if target.exists():
                    digest = hashlib.sha256()
                    with target.open("rb") as stream:
                        for block in iter(lambda: stream.read(8*1024*1024), b""):
                            digest.update(block)
                    if digest.hexdigest() != previous.get("sha256", {}).get(filename):
                        raise ValueError("existing model file changed; preserve it and choose a new download folder")
        dest.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ, HF_HUB_OFFLINE="0", TRANSFORMERS_OFFLINE="0")
        result = command([shutil.which("hf") or "hf", "download", repo, *filenames, "--revision", revision,
                          "--local-dir", str(dest), "--max-workers", "1"], work, env, timeout=24*3600)
        if result["exit_code"] != 0:
            return {"exit_code": result["exit_code"], "error": "Model download failed; check access/license or network. No token was logged."}
        hashes = {}
        for filename in filenames:
            path = confined(dest, filename)
            if path.stat().st_size != available[filename].get("size", available[filename].get("lfs", {}).get("size")):
                raise ValueError("downloaded model size mismatch")
            hasher = hashlib.sha256()
            with path.open("rb") as stream:
                for block in iter(lambda: stream.read(8*1024*1024), b""):
                    hasher.update(block)
            hashes[filename] = hasher.hexdigest()
            expected = available[filename].get("lfs", {}).get("sha256")
            if expected and hashes[filename] != expected:
                raise ValueError("downloaded model hash mismatch")
        provenance.write_text(json.dumps({"repo": repo, "revision": revision, "sha256": hashes}, indent=2))
        return {"destination": str(dest.relative_to(ROOT)), "revision": revision, "sha256": hashes}
    if action == "model_quantize":
        source = confined(ROOT / "models", args["source"])
        dest = confined(ROOT / "models", args["destination"])
        if args.get("format") == "mlx":
            if not source.is_dir() or dest.exists() or int(args.get("bits", 4)) not in (2, 4, 8):
                raise ValueError("MLX quantization requires a local model directory, a new destination, and 2/4/8 bits")
            total = sum(p.stat().st_size for p in source.rglob("*.safetensors"))
            if total == 0 or shutil.disk_usage(ROOT).free < total + 5*1024**3:
                raise ValueError("missing Safetensors or insufficient disk reserve for conversion")
            return command([str(ROOT / ".venv-inference/bin/python"), "-m", "mlx_lm", "convert",
                            "--hf-path", str(source), "--mlx-path", str(dest), "-q", "--q-bits", str(int(args.get("bits", 4)))],
                           work, dict(os.environ, HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1"), timeout=24*3600)
        quant = args.get("type", "Q4_K_M")
        if quant not in ("Q4_K_M", "Q5_K_M", "Q3_K_M", "Q2_K") or source.suffix != ".gguf" or dest.suffix != ".gguf" or dest.exists():
            raise ValueError("choose a supported quantization and a NEW GGUF destination; originals are preserved")
        if shutil.disk_usage(ROOT).free < source.stat().st_size + 5*1024**3:
            raise ValueError("insufficient disk to quantize while retaining original")
        dest.parent.mkdir(parents=True, exist_ok=True)
        return command([shutil.which("llama-quantize") or "llama-quantize", str(source), str(dest), quant], work, timeout=24*3600)
    if action == "email_list":
        return mailbox(args)
    if action == "schedule":
        import company_schedule
        return company_schedule.operation(company, run, args)
    if action == "email_status":
        if os.getenv("GENTS_EMAIL_PROVIDER") != "resend" or not re.fullmatch(r"[a-f0-9-]{36}", args.get("id", "")):
            raise ValueError("email_status needs a Resend email UUID")
        message = resend("/emails/" + args["id"])
        if (not message.get("to") or not {a.strip().lower() for a in message["to"]}.issubset(allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS"))
                or email.utils.parseaddr(message.get("from", ""))[1].lower() != email.utils.parseaddr(os.getenv("GENTS_EMAIL_FROM", ""))[1].lower()):
            raise ValueError("email is outside the configured sender/recipient policy")
        return {key: message.get(key) for key in ("id", "to", "from", "subject", "last_event", "scheduled_at")}
    if action == "email_send":
        recipient = args["to"].strip().lower()
        if recipient not in allowed_addresses("GENTS_EMAIL_ALLOWED_RECIPIENTS"):
            raise ValueError("recipient must be in GENTS_EMAIL_ALLOWED_RECIPIENTS")
        sender = os.environ["GENTS_EMAIL_FROM"]
        payload = {"from": sender, "to": [recipient], "subject": args["subject"], "text": args["text"]}
        if args.get("scheduled_at"):
            when = dt.datetime.fromisoformat(args["scheduled_at"].replace("Z", "+00:00"))
            if when.tzinfo is None or not dt.timedelta(0) < when - dt.datetime.now(dt.timezone.utc) <= dt.timedelta(days=30):
                raise ValueError("scheduled_at must be an explicit ISO8601 timestamp with offset, in the next 30 days; never silently send a missed schedule immediately")
            if os.getenv("GENTS_EMAIL_PROVIDER") != "resend":
                raise ValueError("scheduled delivery requires Resend")
            payload["scheduled_at"] = when.isoformat()
        if os.getenv("GENTS_EMAIL_PROVIDER") == "resend":
            return resend("/emails", "POST", payload, args["idempotency_key"])
        message = EmailMessage()
        message["From"], message["To"], message["Subject"] = sender, recipient, args["subject"]
        message["Message-ID"] = f"<{args['idempotency_key']}@gents.local>"
        message.set_content(args["text"])
        with smtplib.SMTP_SSL(os.environ["GENTS_SMTP_HOST"], int(os.getenv("GENTS_SMTP_PORT", "465")), context=tls_context()) as client:
            client.login(os.environ["GENTS_EMAIL_USER"], os.environ["GENTS_EMAIL_PASSWORD"])
            client.send_message(message)
        return {"sent": True, "to": recipient, "message_id": message["Message-ID"]}
    if action == "training_prepare":
        from train_role import samples, digest
        training = __import__("uuid").uuid4().hex
        # UUIDs are used by the existing training admission API.
        training = str(__import__("uuid").UUID(training))
        dataset = confined(ROOT, f"data/companies/{company}/training/datasets/{training}")
        dataset.mkdir(parents=True)
        for split in ("train", "valid", "test"):
            if args.get("dataset_path"):
                source = confined(work, args["dataset_path"]) / f"{split}.jsonl"
                if source.is_symlink() or source.stat().st_size > 10*1024*1024:
                    raise ValueError("reviewed dataset must be regular JSONL under 10 MiB")
                shutil.copyfile(source, dataset / f"{split}.jsonl")
            else:
                (dataset / f"{split}.jsonl").write_text("\n".join(json.dumps(row) for row in args["samples"][split]) + "\n")
        rows = samples(dataset, company, args["role"])
        return {"manifest": {"company_id": company, "training_id": training, "role": args["role"],
                             "base_model": args["base_model"], "dataset_sha256": {s: digest(dataset / f"{s}.jsonl") for s in rows},
                             "dataset_counts": {s: len(rows[s]) for s in rows}, "iterations": min(max(int(args.get("iterations", 100)), 1), 1000),
                             "sequence_length": 512, "smoke": False, "reviewed_dataset": True}}
    if action == "training_status":
        path = confined(ROOT, f"data/companies/{company}/training/runs/{args['training_id']}")
        return {name: json.loads((path / name).read_text()) for name in ("progress.json", "metrics.json") if (path / name).is_file()}
    if action == "process_stop":
        pid = int(args["pid"])
        if pid <= 1 or pid in (os.getpid(), os.getppid()):
            raise ValueError("cannot stop system or harness processes")
        identity = " ".join(subprocess.run(["ps", "-p", str(pid), "-o", "uid=,lstart=,comm="], capture_output=True, text=True).stdout.split())
        if identity != " ".join(args["identity"].split()) or not identity or identity.split()[0] != str(os.getuid()):
            raise ValueError("process identity changed or process belongs to another user")
        if args.get("operator_confirmed") is not True:
            raise ValueError("non-company processes require exact operator confirmation")
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
        result = execute(args.company, args.run, request["action"], request["arguments"])
        print(json.dumps({"ok": True, "result": result}, ensure_ascii=True))
    except Exception as error:
        # Controlled errors omit credentialed HTTP bodies and SMTP/IMAP auth details.
        detail = str(error) if isinstance(error, (ValueError, KeyError, FileNotFoundError)) else type(error).__name__
        print(json.dumps({"ok": False, "error": detail}))
