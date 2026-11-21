import unittest

from audio_timing_gate import compare_timing


def case(key, seconds):
    return {"id": key, "reference_seconds": 10.0, "predicted_seconds": seconds}


class GateTests(unittest.TestCase):
    def test_mean_gain_cannot_hide_regression(self):
        result = compare_timing(
            [case("a", 20), case("b", 10)], [case("a", 10), case("b", 11)]
        )
        self.assertLess(result["candidate_mean_error"], result["baseline_mean_error"])
        self.assertEqual(result["regressions"], ["b"])
        self.assertFalse(result["measured_improvement"])

    def test_bad_inputs_rejected(self):
        for candidate in (
            [],
            [case("b", 10)],
            [case("a", float("nan"))],
            [case("a", True)],
            [case("a", 10), case("a", 10)],
        ):
            with self.assertRaises(ValueError):
                compare_timing([case("a", 12)], candidate)

    def test_improvement_is_not_production_qualification(self):
        result = compare_timing([case("a", 12)], [case("a", 11)])
        self.assertTrue(result["measured_improvement"])
        self.assertFalse(result["production_qualified"])


if __name__ == "__main__":
    unittest.main()
