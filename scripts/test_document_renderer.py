import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
import uuid

import document_renderer


class DocumentRendererTests(unittest.TestCase):
    def spec(self):
        return {"title": "Renderer fixture", "pages": [{"title": "Generic diagram fixture", "text": ["A rectangle, ellipse and label exercise rendering only."], "drawing": [
            {"kind": "rect", "x": 50, "y": 40, "width": 160, "height": 80, "fill": "#e0f2f1"},
            {"kind": "ellipse", "x": 280, "y": 40, "width": 80, "height": 80},
            {"kind": "label", "x": 50, "y": 150, "text": "Generic shapes"}]}]}

    def test_real_pdf_parses_and_renders_without_domain_content(self):
        import pdf_tools
        from pypdf import PdfReader
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / "fixture.json", root / "fixture.pdf"
            source.write_text(json.dumps(self.spec()))
            receipt = document_renderer.render(source, output)
            self.assertFalse(receipt["quality_verified"])
            self.assertEqual(receipt["page_count"], 1)
            self.assertIn("Generic shapes", PdfReader(output).pages[0].extract_text())
            inspection = pdf_tools.inspect(output, root / "renders", 1, 1)
            self.assertTrue(inspection["rendered"])
            self.assertEqual(receipt["sha256"], inspection["sha256"])

    def test_bad_geometry_and_executable_fields_are_rejected(self):
        for shape in [
            {"kind": "rect", "x": 500, "y": 0, "width": 100, "height": 20},
            {"kind": "line", "x": float("nan"), "y": 0, "x2": 20, "y2": 20},
            {"kind": "label", "x": 0, "y": 0, "text": "example", "exec": "anything"},
            {"kind": "polygon", "points": [[0, 0], [1, 1]]},
        ]:
            spec = self.spec(); spec["pages"][0]["drawing"] = [shape]
            with self.assertRaises(ValueError): document_renderer.validate(spec)

    def test_invalid_revision_cannot_replace_a_good_pdf(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / "fixture.json", root / "fixture.pdf"
            source.write_text(json.dumps(self.spec()))
            document_renderer.render(source, output)
            before = output.read_bytes()
            spec = self.spec(); spec["pages"][0]["title"] = "W" * 100
            source.write_text(json.dumps(spec))
            with self.assertRaises(ValueError): document_renderer.render(source, output)
            self.assertEqual(before, output.read_bytes())

    def test_actual_workspace_broker_can_render_inspect_and_confine_paths(self):
        import company_tools
        run = str(uuid.uuid4())
        root = company_tools.ROOT / "data/companies/renderer-selftest/runs" / run
        try:
            company_tools.execute("renderer-selftest", run, "files", {"op":"write", "path":"fixture.json", "content":json.dumps(self.spec())})
            result = company_tools.execute("renderer-selftest", run, "pdf_render", {"source":"fixture.json", "output":"fixture.pdf"})
            inspection = company_tools.execute("renderer-selftest", run, "pdf_inspect", {"path":"fixture.pdf", "page_count":1})
            self.assertEqual(result["sha256"], inspection["sha256"])
            self.assertEqual(result["source"], "fixture.json")
            with self.assertRaises(ValueError):
                company_tools.execute("renderer-selftest", run, "pdf_render", {"source":"../fixture.json", "output":"fixture.pdf"})
            if os.getenv("GENTS_RENDERER_TEST_EVIDENCE"):
                destination = Path(os.environ["GENTS_RENDERER_TEST_EVIDENCE"])
                destination.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(root / "workspace/fixture.pdf", destination / "fixture.pdf")
                shutil.copyfile(root / "workspace" / inspection["inspected_pages"][0]["render"], destination / "fixture.png")
        finally:
            if root.is_dir(): shutil.rmtree(root)


if __name__ == "__main__":
    unittest.main()
