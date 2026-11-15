import json
import datetime as dt
import os
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch
import uuid
import sys
import company_tools as tools


class CompanyTools(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="gents-tools-")
        self.root = Path(self.temp.name).resolve()
        (self.root / "models").mkdir()
        self.patcher = patch.object(tools, "ROOT", self.root)
        self.patcher.start()
        self.run = str(uuid.uuid4())
    def tearDown(self):
        self.patcher.stop()
        self.temp.cleanup()
    def tool(self, action, **args):
        return tools.execute("one", self.run, action, args)
    def test_integer_arithmetic_is_exact_bounded_and_not_code_execution(self):
        result = self.tool("integer_arithmetic", calculations=[{"id":"one","values":[100,20,-7],"subtract":130},{"id":"empty","values":[],"subtract":-9}])
        self.assertEqual(result["calculations"], [{"id":"one","sum":113,"difference":-17},{"id":"empty","sum":0,"difference":9}])
        for value in (True, 1.2, "1+2", 10_000_001):
            with self.assertRaises(ValueError):
                self.tool("integer_arithmetic", calculations=[{"id":"one","values":[value],"subtract":0}])
        with self.assertRaises(ValueError):
            self.tool("integer_arithmetic", calculations=[{"id":"one","values":[],"subtract":0}]*2)
    def test_files_cannot_escape_or_follow_symlinks(self):
        result = self.tool("files", op="write", path="answer.txt", content="verified")
        self.assertEqual((self.root / result["path"]).read_text(), "verified")
        for path in ("../two/secret", "/etc/passwd"):
            with self.assertRaises(ValueError):
                self.tool("files", op="read", path=path)
        work = self.root / f"data/companies/one/runs/{self.run}/workspace"
        (work / "alias").symlink_to(self.root / "models")
        with self.assertRaises(ValueError):
            self.tool("files", op="read", path="alias/private")
    def test_legacy_files_do_not_silently_ignore_native_write_preconditions(self):
        result = self.tool("files", op="write", path="answer.txt", content="original")
        with self.assertRaisesRegex(ValueError, "native MCP"):
            self.tool("files", op="write", path="answer.txt", content="replacement", expected_sha256="a"*64)
        self.assertEqual((self.root / result["path"]).read_text(), "original")
    @unittest.skipUnless(sys.platform == "darwin" and Path("/usr/bin/sandbox-exec").exists(), "macOS filesystem sandbox")
    def test_isolated_process_cannot_follow_a_workspace_symlink_to_modify_host_data(self):
        self.tool("files", op="list", path="")
        work = self.root / f"data/companies/one/runs/{self.run}/workspace"
        outside = self.root / "outside.txt"
        outside.write_text("unchanged")
        (work / "alias.txt").symlink_to(outside)
        result = self.tool("shell", command="/usr/bin/printf changed > alias.txt")
        self.assertNotEqual(result["exit_code"], 0)
        self.assertEqual(outside.read_text(), "unchanged")
        result = self.tool("shell", command=f"/bin/ln {outside} hardlink.txt && /bin/cat hardlink.txt")
        self.assertNotEqual(result["exit_code"], 0)
        self.assertEqual(outside.stat().st_nlink, 1)
    def test_web_rejects_private_and_credentialed_hosts(self):
        with self.assertRaises(ValueError):
            tools.public_address("127.0.0.1")
        for url in ("http://example.com", "https://user:password@example.com", "https://example.com:8443"):
            with self.assertRaises(ValueError):
                tools.web_request(url)
    def test_journal_read_cannot_recursively_embed_journal_results(self):
        self.tool("files", op="list", path="")
        run_root = self.root / f"data/companies/one/runs/{self.run}"
        original = {"request": {"action": "web_read"}, "result": {"result": {"ok": True}}}
        (run_root / "tool-000000.json").write_text(json.dumps(original))
        self.assertEqual(self.tool("journal_read", step=0), original)
        (run_root / "tool-000001.json").write_text(json.dumps({"request": {"action": "journal_read"}, "result": original}))
        with self.assertRaisesRegex(ValueError, "original non-journal"):
            self.tool("journal_read", step=1)
    def test_search_does_not_treat_generic_news_homepages_as_topical_evidence(self):
        generic=[{"title":"Latest breaking news", "url":"https://news.example/", "snippet":"World news and headlines"}]
        with self.assertRaisesRegex(ValueError, "off-topic"):
            tools.relevant_search_results("latest AI breakthroughs agents model releases companies new products", generic)
        topical={"title":"Artificial intelligence research announcement", "url":"https://research.example/paper", "snippet":"New benchmark limitations"}
        self.assertEqual(tools.relevant_search_results("AI breakthroughs", generic+[topical]), [topical])
        with self.assertRaisesRegex(ValueError, "too broad"):
            tools.relevant_search_results("latest news", generic)
    def test_download_requires_immutable_revision_and_nonexecutable_files(self):
        with self.assertRaises(ValueError):
            self.tool("model_download", repo="owner/model", revision="main", files=["model.gguf"], destination="test")
        with self.assertRaises(ValueError):
            self.tool("model_download", repo="owner/model", revision="a"*40, files=["malicious.py"], destination="test")
    def test_dataset_pdf_inventory_and_download_validation_are_bounded(self):
        metadata = {"siblings": [
            {"rfilename": "instructions/one.pdf", "size": 42},
            {"rfilename": "instructions/two.PDF", "lfs": {"size": 84}},
            {"rfilename": "README.md", "size": 10},
            {"rfilename": "unknown.pdf"},
        ]}
        self.assertEqual(tools.safe_dataset_pdfs(metadata), [
            {"path": "instructions/one.pdf", "bytes": 42},
            {"path": "instructions/two.PDF", "bytes": 84},
        ])
        with self.assertRaisesRegex(ValueError, "immutable"):
            self.tool("hf_dataset_download", repo="owner/lego", revision="main",
                      files=["instructions/one.pdf"], destination="pdfs")
        with self.assertRaisesRegex(ValueError, "relative .pdf"):
            self.tool("hf_dataset_download", repo="owner/lego", revision="a"*40,
                      files=["../secret.pdf"], destination="pdfs")
    def test_empty_dataset_search_discovers_account_without_exposing_identity_payload(self):
        identity = {"name": "fixture-owner", "email": "private@example.com", "auth": {"token": "do-not-return"}}
        rows = [{"id": "fixture-owner/lego-examples", "sha": "a"*40}, {"id": "fixture-owner/other"}]
        with patch.dict(os.environ, {"HF_TOKEN": "fixture-secret"}, clear=True), patch.object(tools, "web_request", side_effect=[({}, b"[]"), ({}, json.dumps(identity).encode()), ({}, json.dumps(rows).encode())]) as api:
            result = self.tool("hf_dataset_search", query="LEGO architecture instructions")
        self.assertEqual([row["id"] for row in result["datasets"]], ["fixture-owner/lego-examples"])
        self.assertEqual(result["scope"], "authenticated_account_after_empty_query")
        self.assertIn("author=fixture-owner", api.call_args.args[0])
        self.assertNotIn("private@example.com", json.dumps(result))
        self.assertNotIn("fixture-secret", json.dumps(result))
        self.assertNotIn("do-not-return", json.dumps(result))
    def test_authorized_dataset_download_has_scoped_network_access_and_exact_filenames(self):
        name = "examples/source[1].pdf"
        metadata = {"sha": "a"*40, "siblings": [{"rfilename": name, "size": 7}]}
        def download(argv, work, env, timeout):
            self.assertEqual(argv[3], name)
            self.assertNotIn("--include", argv)
            self.assertEqual(env["HF_HUB_OFFLINE"], "0")
            self.assertEqual(os.environ["HF_HUB_OFFLINE"], "1")
            destination = Path(argv[argv.index("--local-dir")+1]) / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(b"fixture")
            return {"exit_code": 0, "output": ""}
        with patch.dict(os.environ, {"HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"}), patch.object(tools, "hf_dataset_metadata", return_value=metadata), patch.object(tools, "command", side_effect=download):
            result = self.tool("hf_dataset_download", repo="owner/examples", revision="a"*40, files=[name], destination="references")
        self.assertEqual(result["files"][0]["bytes"], 7)
        self.assertEqual(result["files"][0]["path"], "references/"+name)
    def test_dataset_inspection_rejects_invented_repositories_after_discovery(self):
        self.tool("files", op="list", path="")
        run_root = self.root / f"data/companies/one/runs/{self.run}"
        record = {"request": {"action": "hf_dataset_search"}, "result": {"result": {"ok": True, "result": {"datasets": [{"id": "fixture-owner/lego-examples"}]}}}}
        (run_root / "tool-000000.json").write_text(json.dumps(record))
        with patch.object(tools, "hf_dataset_metadata", return_value={"id": "fixture-owner/lego-examples", "sha": "a"*40, "siblings": []}) as api:
            with self.assertRaisesRegex(ValueError, "not returned"):
                self.tool("hf_dataset_inspect", repo="invented/lego")
            api.assert_not_called()
            self.assertEqual(self.tool("hf_dataset_inspect", repo="fixture-owner/lego-examples")["revision"], "a"*40)
    def test_inventory_retrieval_pages_exact_observed_paths_without_a_network_call(self):
        self.tool("files", op="list", path="")
        run_root = self.root / f"data/companies/one/runs/{self.run}"
        record = {"result": {"step": 3, "action": "hf_dataset_inspect", "result": {"ok": True, "result": {"repo": "owner/source", "revision": "a"*40, "pdfs": [{"path": "examples/fixture-one.pdf"}, {"path": "examples/fixture-two.pdf"}, {"path": "other.pdf"}]}}}}
        (run_root / "tool-000003.json").write_text(json.dumps(record))
        with patch.object(tools, "web_request", side_effect=AssertionError("inventory retrieval must be local")):
            first = self.tool("hf_dataset_files", repo="owner/source", revision="a"*40, query="FIXTURE", limit=1)
            self.assertEqual(first["pdfs"], [{"path": "examples/fixture-one.pdf"}])
            self.assertEqual(first["total_matches"], 2)
            second = self.tool("hf_dataset_files", repo="owner/source", revision="a"*40, query="fixture", limit=1, offset=first["next_offset"])
            self.assertEqual(second["pdfs"], [{"path": "examples/fixture-two.pdf"}])
            self.assertIsNone(second["next_offset"])
            normalized = self.tool("hf_dataset_files", repo="owner/source", revision="a"*40, query="fixture two")
            self.assertEqual(normalized["pdfs"], [{"path": "examples/fixture-two.pdf"}])
            self.assertEqual(normalized["match_mode"], "literal_or_punctuation_normalized_path")
            with self.assertRaisesRegex(ValueError, "No successful inventory"):
                self.tool("hf_dataset_files", repo="owner/source", revision="b"*40)
    def test_quantization_never_overwrites_an_original(self):
        source = self.root / "models/original.gguf"
        source.write_bytes(b"original")
        with self.assertRaises(ValueError):
            self.tool("model_quantize", source="original.gguf", destination="original.gguf", type="Q2_K")
        self.assertEqual(source.read_bytes(), b"original")
    def test_email_is_disabled_without_explicit_address_configuration(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(ValueError):
                self.tool("email_send", to="somebody@example.com", subject="hello", text="test")
            with self.assertRaises(ValueError):
                self.tool("email_list")
    def test_resend_read_requires_a_dedicated_inbox(self):
        with patch.dict(os.environ, {"GENTS_EMAIL_PROVIDER": "resend", "GENTS_EMAIL_ALLOWED_SENDERS": "owner@example.com"}, clear=True), patch.object(tools, "resend") as api:
            with self.assertRaisesRegex(ValueError, "GENTS_EMAIL_INBOX"):
                tools.mailbox({})
            api.assert_not_called()
    def test_resend_filters_both_sender_and_recipient_and_rechecks_body(self):
        env = {"GENTS_EMAIL_PROVIDER": "resend", "GENTS_EMAIL_ALLOWED_SENDERS": "owner@example.com",
               "GENTS_EMAIL_INBOX": "gents@example.com"}
        allowed = {"id": "one", "from": "Owner <OWNER@example.com>", "to": ["Gents <gents@example.com>"], "text": "task"}
        other_inbox = dict(allowed, id="two", to=["personal@example.com"])
        other_sender = dict(allowed, id="three", **{"from": "stranger@example.com"})
        with patch.dict(os.environ, env, clear=True), patch.object(tools, "resend") as api:
            api.return_value = {"data": [allowed, other_inbox, other_sender]}
            result = tools.mailbox({})
            self.assertEqual(result["messages"], [allowed])
            self.assertTrue(result["untrusted_content"])
            for denied in (other_inbox, other_sender):
                api.return_value = denied
                with self.assertRaisesRegex(ValueError, "inbox policy"):
                    tools.mailbox({"id": denied["id"]})
            api.return_value = allowed
            self.assertEqual(tools.mailbox({"id": "one"})["text"], "task")
    def test_resend_send_uses_configured_identity_recipient_and_idempotency(self):
        env = {"GENTS_EMAIL_PROVIDER": "resend", "GENTS_EMAIL_FROM": "Gents <vcv@example.com>",
               "GENTS_EMAIL_ALLOWED_RECIPIENTS": "owner@example.com", "GENTS_EMAIL_DEFAULT_TO": "owner@example.com"}
        with patch.dict(os.environ, env, clear=True), patch.object(tools, "resend", return_value={"id": "fixture-id"}) as api:
            self.assertEqual(self.tool("email_send", to="OWNER@example.com", subject="Hello", text="Report", idempotency_key="intent-fixture"), {"id": "fixture-id"})
            api.assert_called_once_with("/emails", "POST", {"from": "Gents <vcv@example.com>", "to": ["owner@example.com"], "subject": "Hello", "text": "Report"}, "intent-fixture")
            with self.assertRaises(ValueError):
                self.tool("email_send", to="stranger@example.com", subject="No", text="No", idempotency_key="deny")
            self.assertEqual(api.call_count, 1)
            settings = tools.settings()
            self.assertTrue(settings["email_default_to_configured"])
            self.assertEqual(settings["email_allowed_recipient_count"], 1)
            self.assertNotIn("email_default_to", settings)
            self.assertNotIn("RESEND_API_KEY", settings)
    def test_email_scheduling_does_not_silently_send_past_or_naive_times(self):
        env = {"GENTS_EMAIL_PROVIDER": "resend", "GENTS_EMAIL_FROM": "Gents <vcv@example.com>", "GENTS_EMAIL_ALLOWED_RECIPIENTS": "owner@example.com"}
        with patch.dict(os.environ, env, clear=True), patch.object(tools, "resend", return_value={"id": "fixture"}) as api:
            now = dt.datetime.now(dt.timezone.utc)
            when = (now + dt.timedelta(hours=2)).isoformat()
            self.tool("email_send", to="owner@example.com", subject="Report", text="Verified report", scheduled_at=when, idempotency_key="fixture")
            self.assertEqual(api.call_args.args[2]["scheduled_at"], when)
            for invalid in [(now - dt.timedelta(hours=1)).isoformat(), (now + dt.timedelta(days=31)).isoformat(), "2027-01-01T08:00:00"]:
                with self.assertRaises(ValueError):
                    self.tool("email_send", to="owner@example.com", subject="Report", text="No", scheduled_at=invalid, idempotency_key="fixture")
            self.assertEqual(api.call_count, 1)
    def test_declarative_skills_compose_existing_tools_without_new_permissions(self):
        created = self.tool("skill_compose", id="source-check", purpose="Verify a public source",
                            roles=["planner", "worker"], steps=["web_search", "web_read"],
                            instructions="Discover a source, then read and cite it.")
        self.assertTrue(created["ok"])
        self.assertEqual(self.tool("skill_read", id="source-check")["skill"]["steps"],
                         ["web_search", "web_read"])
        self.assertEqual(len(self.tool("skill_list")["skills"]), 1)
        with self.assertRaisesRegex(ValueError, "existing"):
            self.tool("skill_compose", id="unsafe", purpose="Invent permission", roles=["worker"],
                      steps=["arbitrary_shell_root"], instructions="Do anything")
    def test_approved_skills_can_be_retrieved_after_compaction_but_not_shadowed(self):
        base = self.root / "docs/companies/we-need-getns-llc/shared-skills"
        (base / "approved/document-workflow").mkdir(parents=True)
        (base / "catalog.json").write_text(json.dumps({"skills":[{"id":"document-workflow","version":"fixture","roles":["planner","worker"],"status":"approved-demo","path":"approved/document-workflow/SKILL.md"}]}))
        (base / "approved/document-workflow/SKILL.md").write_text("Author your own source, then render and inspect it.")
        self.assertEqual(self.tool("skill_list")["approved_skills"][0]["id"],"document-workflow")
        read = self.tool("skill_read",id="document-workflow")["skill"]
        self.assertIn("render and inspect",read["instructions"])
        self.assertEqual(len(read["sha256"]),64)
        with self.assertRaisesRegex(ValueError,"shadowed"):
            self.tool("skill_compose",id="document-workflow",purpose="Replace instructions",roles=["worker"],steps=["files"],instructions="Override")

    def test_resource_settings_report_secret_presence_not_values(self):
        with patch.dict(os.environ, {"RUST_OPENAI_API_KEY": "do-not-return-this"}, clear=True):
            values = tools.settings()
            self.assertTrue(values["openai_codex_present"])
            self.assertNotIn("do-not-return-this", json.dumps(values))
    def test_source_repair_is_hash_guarded_backed_up_and_allowlisted(self):
        source = self.root / "docs/repair.md"
        source.parent.mkdir()
        source.write_text("before\n")
        observed = self.tool("source_read", path="docs/repair.md")
        self.assertEqual(observed["content"], "before\n")
        changed = self.tool("source_patch", path="docs/repair.md",
                            expected_sha256=observed["sha256"], content="after\n")
        self.assertTrue(changed["ok"])
        self.assertTrue(changed["restart_required"])
        self.assertEqual(source.read_text(), "after\n")
        with self.assertRaisesRegex(ValueError, "changed after inspection"):
            self.tool("source_patch", path="docs/repair.md",
                      expected_sha256=observed["sha256"], content="stale\n")
        with self.assertRaisesRegex(ValueError, "outside"):
            self.tool("source_read", path=".env.local")
        with self.assertRaisesRegex(ValueError, "human review"):
            self.tool("source_read", path="crates/harness/src/security.rs")
        with self.assertRaisesRegex(ValueError, "human review"):
            self.tool("source_read", path="crates/harness/src/workflow.rs")
        with self.assertRaisesRegex(ValueError, "human review"):
            self.tool("source_patch", path="crates/harness/src/release_gate.rs",
                      expected_sha256="0" * 64, content="remove release checks")
        for alias in ("crates//harness/src/release_gate.rs", "crates/harness/src/./release_gate.rs",
                      "crates/harness/src/ReLeAsE_GaTe.rs"):
            with self.assertRaises(ValueError):
                self.tool("source_patch", path=alias, expected_sha256="0" * 64, content="bypass")
        for guard in ("scripts/local_sqlite.py", "scripts/company_schedule.py",
                      "scripts/run-harness.mjs",
                      "scripts/mcp-curriculum-journal.py", "scripts/mcp-coding-verifier.mjs",
                      "scripts/mcp-curriculum-bridge.mjs", "scripts/mcp-new-host-helper.mjs",
                      "scripts/fixtures/mcp-rust-weighted-slots.mjs",
                      "scripts/FIXTURES/new-reviewed-suite.mjs",
                      "config/mcp-development-curriculum.json",
                      "config/company.mcp-development.json",
                      "config/COMPANY.MCP-rust-reasoning-specialist-review.json",
                      "crates/harness/src/review_context.rs",
                      "crates/harness/src/memory_archive.rs",
                      "crates/harness/src/rust_review.rs",
                      "crates/harness/src/completion_review.rs",
                      "crates/harness/src/authorization_context.rs"):
            with self.assertRaisesRegex(ValueError, "human review"):
                self.tool("source_patch", path=guard, expected_sha256="0"*64, content="bypass")
        linked = source.parent / "linked.md"
        os.link(source, linked)
        with self.assertRaisesRegex(ValueError, "hard-linked"):
            self.tool("source_patch", path="docs/linked.md",
                      expected_sha256=changed["sha256"], content="bypass through hardlink")
        self.assertEqual(source.read_text(), "after\n")
    @unittest.skipUnless(__import__("sys").platform=="darwin", "uses macOS sandbox")
    def test_shell_writes_only_workspace_and_cannot_read_host_data(self):
        output = self.tool("shell", command="/usr/bin/printf verified > result.txt")
        self.assertEqual(output["exit_code"], 0, output)
        work = self.root / f"data/companies/one/runs/{self.run}/workspace"
        self.assertEqual((work / "result.txt").read_text(), "verified")
        private = self.root / "outside.txt"
        private.write_text("host-data")
        output = self.tool("shell", command=f"/bin/cat '{private}'")
        self.assertNotEqual(output["exit_code"], 0)
        self.assertNotIn("host-data", output["output"])
        output = self.tool("shell", command=f"/usr/bin/printf changed > '{private}'")
        self.assertNotEqual(output["exit_code"], 0)
        self.assertEqual(private.read_text(), "host-data")
        output = self.tool("shell", command="/usr/bin/curl --max-time 2 -s https://example.com")
        self.assertNotEqual(output["exit_code"], 0)
        child = subprocess.Popen(["/bin/sleep", "30"])
        try:
            output = self.tool("shell", command=f"/bin/kill -TERM {child.pid}")
            self.assertNotEqual(output["exit_code"], 0, output)
            self.assertIsNone(child.poll(), "sandbox signalled an unrelated process")
        finally:
            child.terminate()
            child.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
