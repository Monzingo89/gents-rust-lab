"""Frozen before candidate generation: conservative public-view token redaction."""

import importlib.util
import json
import os
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "candidate", Path(os.environ["RESEARCH_CANDIDATE"])
)
assert spec is not None
assert spec.loader is not None
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
MARK = "[PUBLIC_TOKEN_REDACTED]"


class Acceptance(unittest.TestCase):
    def test_public_contact_token_removed_without_mapping(self):
        value = {
            "source": "https://docs.python.org/",
            "text": "See person@example.invalid for details.",
        }
        out, count = m.redact_public_at_tokens(value)
        self.assertEqual(
            out, {"source": value["source"], "text": "See " + MARK + " for details."}
        )
        self.assertEqual(count, 1)
        self.assertNotIn("person", json.dumps(out))
        self.assertEqual(value["text"], "See person@example.invalid for details.")

    def test_decorator_and_ambiguous_at_tokens_are_lossily_removed(self):
        out, n = m.redact_public_at_tokens(
            {
                "text": 'code\n@unittest.skip("reason")\nnext a@@b mailto:x@example.invalid'
            }
        )
        self.assertEqual(n, 3)
        self.assertEqual(out["text"], "code\n" + MARK + "\nnext " + MARK + " " + MARK)

    def test_nested_json_and_types_preserved(self):
        value = {
            "items": [{"snippet": "x@y.invalid", "ok": True}, None, 3, 2.5, False],
            "text": "normal",
        }
        out, n = m.redact_public_at_tokens(value)
        self.assertEqual(n, 1)
        self.assertEqual(
            out["items"], [{"snippet": MARK, "ok": True}, None, 3, 2.5, False]
        )
        self.assertEqual(out["text"], "normal")
        self.assertEqual(value["items"][0]["snippet"], "x@y.invalid")

    def test_whitespace_is_preserved_including_unicode(self):
        out, n = m.redact_public_at_tokens(
            "a\tfoo@bar.invalid\r\nx\u2003z@q.invalid\u00a0end"
        )
        self.assertEqual(out, "a\t" + MARK + "\r\nx\u2003" + MARK + "\u00a0end")
        self.assertEqual(n, 2)

    def test_unsafe_keys_and_non_json_values_fail_without_echo(self):
        for value in [
            {"secret@domain.invalid": "v"},
            {3: "v"},
            (1, 2),
            object(),
            float("nan"),
            float("inf"),
        ]:
            with self.assertRaises((ValueError, TypeError)) as ctx:
                m.redact_public_at_tokens(value)
            self.assertNotIn("secret@", str(ctx.exception))

    def test_clean_payload_is_value_idempotent_and_independent(self):
        original = {"a": [{"x": "safe"}]}
        first, n = m.redact_public_at_tokens(original)
        second, k = m.redact_public_at_tokens(first)
        self.assertEqual((first, n), (original, 0))
        self.assertEqual((second, k), (first, 0))
        first["a"][0]["x"] = "changed"
        self.assertEqual(original["a"][0]["x"], "safe")
        self.assertEqual(second["a"][0]["x"], "safe")

    def test_repeat_calls_do_not_retain_other_payloads(self):
        self.assertEqual(m.redact_public_at_tokens("a@b.invalid"), (MARK, 1))
        self.assertEqual(m.redact_public_at_tokens("clean"), ("clean", 0))

    def test_punctuation_and_multiple_tokens_are_completely_removed(self):
        out, n = m.redact_public_at_tokens('("a@b.invalid"), other <c@d.invalid>')
        self.assertEqual(out, MARK + " other " + MARK)
        self.assertEqual(n, 2)

    def test_large_at_free_token_and_long_contact_are_bounded(self):
        text = "x" * 300000
        self.assertEqual(m.redact_public_at_tokens(text), (text, 0))
        self.assertEqual(m.redact_public_at_tokens(text + "@domain.invalid"), (MARK, 1))

    def test_boundary_does_not_make_other_identifiers_safe(self):
        value = {"text": "non-contact content 123-45-6789"}
        out, n = m.redact_public_at_tokens(value)
        self.assertEqual(out, value)
        self.assertEqual(n, 0)


if __name__ == "__main__":
    unittest.main()
