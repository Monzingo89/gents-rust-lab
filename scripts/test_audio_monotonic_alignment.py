import itertools
import unittest

import numpy as np

from audio_monotonic_alignment import maximum_path


class AlignmentTests(unittest.TestCase):
    def test_matches_exhaustive_optimum(self):
        rng = np.random.default_rng(17)
        for frames in range(1, 8):
            for tokens in range(1, frames + 1):
                scores = rng.normal(size=(frames, tokens))
                alternatives = []
                for boundaries in itertools.combinations(range(1, frames), tokens - 1):
                    indices = np.searchsorted(boundaries, np.arange(frames), side='right')
                    alternatives.append(scores[np.arange(frames), indices].sum())
                path = maximum_path(scores)
                self.assertAlmostEqual(float((scores * path).sum()), max(alternatives))
                np.testing.assert_array_equal(path.sum(1), np.ones(frames))
                self.assertTrue((path.sum(0) >= 1).all())
                self.assertEqual(path[0, 0], 1)
                self.assertEqual(path[-1, -1], 1)
                np.testing.assert_array_equal(path, maximum_path(scores))

    def test_invalid_inputs(self):
        for scores in (np.zeros((2, 3)), np.zeros((0, 0)), [[float('nan')]], [1, 2]):
            with self.assertRaises(ValueError):
                maximum_path(scores)


if __name__ == '__main__':
    unittest.main()
