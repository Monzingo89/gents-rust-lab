import json
import datetime as dt
import os
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch
import uuid
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
    def test_web_rejects_private_and_credentialed_hosts(self):
        with self.assertRaises(ValueError):
            tools.public_address("127.0.0.1")
        for url in ("http://example.com", "https://user:password@example.com", "https://example.com:8443"):
            with self.assertRaises(ValueError):
                tools.web_request(url)
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
            self.assertEqual(settings["email_default_to"], "owner@example.com")
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
