import json
import tempfile
import unittest
from pathlib import Path
from PIL import Image,ImageDraw
from prepare_image_count_experiment import prepare
from image_count_evaluation import measure


class CountingTests(unittest.TestCase):
    def test_procedural_counts_and_wrong_request(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/"fixture";prepare(root)
            criteria=json.loads((root/"experiment.json").read_text())["criteria"]
            for count in range(2,8):
                with Image.open(root/f"train/green-{count}.png") as image:
                    case={"color":"green","count":count}
                    self.assertTrue(measure(image,case,criteria)["passed"])
                    case["count"]=count+1
                    self.assertFalse(measure(image,case,criteria)["passed"])
            image=Image.new("RGB",(256,256),"white")
            ImageDraw.Draw(image).rectangle((30,30,100,65),fill=(30,180,60))
            self.assertFalse(measure(image,{"color":"green","count":2},criteria)["passed"])
