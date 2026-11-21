"""Pure evaluation-contract tests: never start a model or backend."""

import unittest

from paired_campaign import cases, expected


class EvaluationContractTests(unittest.TestCase):
    def test_seed_is_reproducible_but_changes_amounts(self):
        first = cases(2731, "train")
        self.assertEqual(first, cases(2731, "train"))
        self.assertNotEqual(first, cases(91821, "train"))
        self.assertEqual(len(first), 10)
        self.assertEqual(len({row["case_id"] for row in first}), 10)

    def test_empty_and_currency_priority_remain_independent(self):
        records = cases(2731, "train")
        self.assertEqual(expected(records[9])["status"], "missing")
        self.assertEqual(expected(records[9])["difference_cents"], 0)
        combined = expected(records[3])
        self.assertTrue(combined["currency_mismatch"])
        self.assertTrue(combined["duplicate_ids"])
        self.assertEqual(combined["status"], "invalid_currency")

    def test_duplicate_and_negative_amounts_are_not_discarded(self):
        record = cases(2731, "train")[2]
        result = expected(record)
        self.assertLess(record["payments"][2]["cents"], 0)
        self.assertEqual(
            result["observed_cents"], sum(p["cents"] for p in record["payments"])
        )
        self.assertEqual(result["status"], "duplicate")

    def test_fresh_holdout_preserves_full_status_coverage(self):
        for seed in (17, 93615, 734252):
            records = cases(seed, "heldout")
            self.assertEqual(
                {expected(row)["status"] for row in records},
                {"matched", "mismatch", "duplicate", "invalid_currency", "missing"},
            )


if __name__ == "__main__":
    unittest.main()
