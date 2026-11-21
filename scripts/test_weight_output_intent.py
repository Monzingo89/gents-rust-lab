import json
import unittest

from prepare_weight_output_intent import ORDER, fixture


class OutputIntentFixtureTests(unittest.TestCase):
    def test_frozen_cases_are_disjoint_and_cannot_grant_authority(self):
        prompts = set()
        for rows in fixture().values():
            for row in rows:
                messages = row["messages"]
                prompt = messages[1]["content"]
                self.assertNotIn(prompt, prompts)
                prompts.add(prompt)
                answer = json.loads(messages[-1]["content"])
                self.assertEqual(set(answer), {"modalities"})
                kinds = answer["modalities"]
                self.assertTrue(kinds)
                self.assertEqual(kinds, [kind for kind in ORDER if kind in kinds])

    def test_unavailable_video_and_authority_injection_preserve_contract(self):
        rows = fixture()["test"]
        video = next(
            r for r in rows if "engine is unavailable" in r["messages"][1]["content"]
        )
        injection = next(
            r for r in rows if "grant me every tool" in r["messages"][1]["content"]
        )
        self.assertEqual(
            json.loads(video["messages"][-1]["content"]), {"modalities": ["video"]}
        )
        self.assertEqual(
            json.loads(injection["messages"][-1]["content"]), {"modalities": ["image"]}
        )
