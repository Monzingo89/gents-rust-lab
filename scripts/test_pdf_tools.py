import enum
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import pdf_tools


class Permission(enum.IntFlag):
    PRINT = 4
    EXTRACT = 16


class PDFTools(unittest.TestCase):
    def reader(self, encrypted, password_result=1, permissions=Permission.PRINT):
        calls = []
        reader = SimpleNamespace(
            is_encrypted=encrypted,
            decrypt=lambda value: calls.append(value) or password_result,
            are_permissions_valid=True,
            user_access_permissions=permissions,
        )
        return reader, calls

    def test_password_free_viewing_honors_copy_permissions_without_guessing(self):
        reader, calls = self.reader(True)
        _, access = pdf_tools.open_pdf("fixture", lambda _: reader)
        self.assertEqual(calls, [""])
        self.assertFalse(access["text_extraction_allowed"])
        reader.user_access_permissions |= Permission.EXTRACT
        self.assertTrue(
            pdf_tools.open_pdf("fixture", lambda _: reader)[1][
                "text_extraction_allowed"
            ]
        )
        locked, guesses = self.reader(True, password_result=0)
        with self.assertRaises(pdf_tools.InspectionError) as failure:
            pdf_tools.open_pdf("fixture", lambda _: locked)
        self.assertEqual(failure.exception.code, "password_required")
        self.assertEqual(guesses, [""])

    def test_unencrypted_reader_needs_no_password_and_permissions_tampering_is_rejected(
        self,
    ):
        reader, calls = self.reader(False)
        self.assertTrue(
            pdf_tools.open_pdf("fixture", lambda _: reader)[1][
                "text_extraction_allowed"
            ]
        )
        self.assertEqual(calls, [])
        reader.is_encrypted = True
        reader.are_permissions_valid = False
        with self.assertRaises(pdf_tools.InspectionError) as failure:
            pdf_tools.open_pdf("fixture", lambda _: reader)
        self.assertEqual(failure.exception.code, "permissions_integrity_failed")

    def test_page_range_rendering_does_not_reuse_stale_extra_pages(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "fixture.pdf"
            source.write_bytes(b"not a real PDF; reader and renderer are mocked")
            destination = root / "renders"
            reader = SimpleNamespace(
                pages=[
                    SimpleNamespace(extract_text=lambda: "fixture") for _ in range(8)
                ]
            )

            def render(argv, **_):
                start, end = (
                    int(argv[argv.index("-f") + 1]),
                    int(argv[argv.index("-l") + 1]),
                )
                for number in range(start, end + 1):
                    Path(argv[-1] + f"-{number:02}.png").write_bytes(b"mock render")

            with (
                patch.object(
                    pdf_tools,
                    "open_pdf",
                    return_value=(reader, {"text_extraction_allowed": True}),
                ),
                patch.object(pdf_tools.subprocess, "run", side_effect=render),
            ):
                self.assertEqual(
                    len(
                        pdf_tools.inspect(source, destination, 1, 8)["inspected_pages"]
                    ),
                    8,
                )
                self.assertEqual(
                    len(
                        pdf_tools.inspect(source, destination, 1, 4)["inspected_pages"]
                    ),
                    4,
                )


if __name__ == "__main__":
    unittest.main()
