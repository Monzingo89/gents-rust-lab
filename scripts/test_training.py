import json
import tempfile
import unittest
from pathlib import Path

from train_role import confined, samples


class TrainingBoundaries(unittest.TestCase):
    def test_training_profiles_are_bounded_and_copied(self):
        from weight_training_profiles import profile

        self.assertEqual(profile("four-layer-v1")["layers"], 4)
        modified = profile()
        modified["layers"] = 999
        self.assertEqual(profile()["layers"], 1)
        for value in ["remote", None, {"layers": 999}]:
            with self.assertRaises(ValueError):
                profile(value)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="gents-training-test-")
        self.root = Path(self.temporary.name)
        for split, tag in [("train", "one"), ("valid", "two"), ("test", "three")]:
            rows = [self.row(tag + str(i)) for i in range(2 if split == "train" else 1)]
            (self.root / f"{split}.jsonl").write_text(
                "\n".join(json.dumps(r) for r in rows)
            )

    def tearDown(self):
        self.temporary.cleanup()

    def row(self, prompt):
        return {
            "company_id": "one",
            "role": "backend-engineer",
            "reviewed": True,
            "license": "CC0-1.0",
            "source": "synthetic",
            "messages": [
                {"role": "user", "content": prompt},
                {"role": "assistant", "content": "ok"},
            ],
        }

    def test_valid_smoke_is_not_a_production_dataset(self):
        self.assertEqual(
            len(samples(self.root, "one", "backend-engineer", True)["train"]), 2
        )
        with self.assertRaises(ValueError):
            samples(self.root, "one", "backend-engineer")

    def test_cross_company_and_role_examples_are_rejected(self):
        for company, role in [("two", "backend-engineer"), ("one", "law")]:
            with self.assertRaises(ValueError):
                samples(self.root, company, role, True)

    def test_held_out_prompt_cannot_be_in_training(self):
        (self.root / "test.jsonl").write_text(json.dumps(self.row("one0")))
        with self.assertRaisesRegex(ValueError, "duplicate prompt"):
            samples(self.root, "one", "backend-engineer", True)

    def test_owner_evaluation_and_unreviewed_examples_are_rejected(self):
        for field, value in [
            ("reviewed", False),
            ("source", "owner-evaluation/private.txt"),
        ]:
            item = self.row("new")
            item[field] = value
            (self.root / "test.jsonl").write_text(json.dumps(item))
            with self.assertRaises(ValueError):
                samples(self.root, "one", "backend-engineer", True)

    def test_symlinks_and_traversal_are_rejected(self):
        (self.root / "link").symlink_to(self.root / "train.jsonl")
        for path in [self.root / "link", self.root / "../escape"]:
            with self.assertRaises(ValueError):
                confined(self.root, path)


if __name__ == "__main__":
    unittest.main()
