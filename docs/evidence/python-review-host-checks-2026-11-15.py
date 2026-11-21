import random
import unittest

import candidate


class HostCases(unittest.TestCase):
    def test_contract_examples(self):
        for values, expected in [
            ([], []),
            (["b", "a", "b"], ["b", "a"]),
            (["A", "a", "A"], ["A", "a"]),
            (["β", "😀", "β"], ["β", "😀"]),
            (["", "x", ""], ["", "x"]),
            (["z"], ["z"]),
        ]:
            saved = values[:]
            self.assertEqual(candidate.unique_stable(values), expected)
            self.assertEqual(values, saved)

    def test_deterministic_generated_lists(self):
        rng = random.Random(20261115)
        for _ in range(300):
            values = [
                rng.choice(["", "A", "a", "β", "😀", "two words", "x"])
                for _ in range(rng.randrange(80))
            ]
            expected = [v for i, v in enumerate(values) if values.index(v) == i]
            saved = values[:]
            self.assertEqual(candidate.unique_stable(values), expected)
            self.assertEqual(values, saved)
