import unittest

from model_capability_report import classify


class ClassificationTests(unittest.TestCase):
    def test_regression_overrides_aggregate_gain(self):
        self.assertEqual(classify(2, 3, 4, 1), "rejected_regression")
        self.assertEqual(classify(2, 2, 4, 0), "no_measured_gain")
        self.assertEqual(classify(2, 3, 4, 0), "narrow_measured_gain_unqualified")

    def test_invalid_counts_fail_closed(self):
        for values in [(0, 1, 0, 0), (True, 1, 2, 0), (1, 3, 2, 0), (1, 1, 2, -1)]:
            with self.assertRaises(ValueError):
                classify(*values)


if __name__ == "__main__":
    unittest.main()
