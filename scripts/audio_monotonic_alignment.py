"""CPU monotonic alignment: each frame stays on a token or advances one token.

Independent development adapter, not installed into or patched over Piper.
Scores have shape [frames, tokens]; every token receives at least one frame.
"""
import numpy as np


def maximum_path(scores):
    scores = np.asarray(scores, dtype=np.float64)
    if scores.ndim != 2 or not np.isfinite(scores).all():
        raise ValueError('expected a finite frame-by-token matrix')
    frames, tokens = scores.shape
    if not 0 < tokens <= frames or frames * tokens > 2_000_000:
        raise ValueError('invalid or excessive alignment dimensions')
    previous = np.full(tokens, -np.inf)
    previous[0] = scores[0, 0]
    advance = np.zeros((frames, tokens), dtype=bool)
    for frame in range(1, frames):
        moved = np.concatenate(([-np.inf], previous[:-1]))
        # A tie stays on the current token, making backtracking deterministic.
        advance[frame] = moved > previous
        current = np.maximum(previous, moved) + scores[frame]
        current[min(frame + 1, tokens):] = -np.inf
        previous = current
    path = np.zeros((frames, tokens), dtype=np.float32)
    token = tokens - 1
    for frame in range(frames - 1, -1, -1):
        path[frame, token] = 1
        if advance[frame, token]:
            token -= 1
    if token != 0:
        raise ValueError('alignment did not reach first token')
    return path
