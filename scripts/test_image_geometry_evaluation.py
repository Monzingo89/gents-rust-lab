import tempfile
import unittest
from pathlib import Path

from image_geometry_evaluation import measure
from PIL import Image, ImageDraw
from prepare_image_weight_experiment import prepare


class GeometryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "fixture"
        self.manifest = prepare(self.root)
        self.criteria = self.manifest["criteria"]

    def test_owned_reference_passes_and_wrong_color_fails(self):
        with Image.open(self.root / "train/red-112.png") as image:
            self.assertTrue(measure(image, "red", self.criteria)["passed"])
            self.assertFalse(measure(image, "blue", self.criteria)["passed"])

    def test_empty_offcenter_and_non_square_fail(self):
        for box in [None, (0, 0, 111, 111), (72, 100, 183, 135)]:
            image = Image.new("RGB", (256, 256), "white")
            if box:
                ImageDraw.Draw(image).rectangle(box, fill=(220, 30, 30))
            self.assertFalse(measure(image, "red", self.criteria)["passed"])
