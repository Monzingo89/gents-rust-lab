# Local model improvement scorecard

Generate the current read-only evidence index from the repository root:

```sh
.venv-media/bin/python scripts/model_capability_report.py > data/model-registry/capability-report.json
```

This scans installed model directories and indexes the retained experiment evidence
with SHA-256 digests. It does not run inference, train models, grant admission or
prove that an old execution can still be reproduced. Missing evidence fails the
command instead of silently yielding a green result. Different datasets remain
separate; there is no fabricated company-wide intelligence score.

The November 14 inventory contains **19 artifact directories**, including formats
of the same base model. They are not 19 distinct model identities. Three directories
have weight experiments: Gemma E4B, FLUX Klein and Piper LJ Speech. Sixteen have no
indexed before/after improvement experiment. There are **zero qualified production
improvements** in this experiment series.

| Experiment | Measured result | Decision |
| --- | --- | --- |
| Gemma simple extraction | 6/6 → 6/6 | No gain |
| Gemma event folding | 1/12 → 2/12 | Narrow gain; unqualified |
| Gemma diverse events | 10/24 → 9/24 | Rejected regression |
| Gemma four-layer reconciliation | 3/24 → 4/24 | Narrow gain; unqualified |
| Gemma strict output-intent format | 0/24 → 11/24 | Format gain; diagnostic label accuracy unchanged at 23/24 |
| FLUX counting | 2/4 → 2/4 | No gain |
| Piper duration prediction | Test mean error 8.56% → 7.83% | Rejected individual regression |
| Piper weight interpolation | Three validation means improved | All rejected individual regression |

The index is a research record, not the runtime model security registry. In
particular, an existing operational model can still lack evidence of improvement.
All indexed candidates remain unpromoted. Provider fees for these experiments were
$0; local electricity and hardware costs were not measured.

Next coverage gaps are the remaining language models, visual understanding,
general image quality, speech intelligibility/naturalness, and video generation.
No registered video-generation backend is currently identified by the inventory.
New candidates need frozen before/after cases, saved-artifact reload checks and
regression rejection. Reusing observed test cases to choose a new method requires
fresh held-out qualification afterward.

See [text experiment details](129-local-weight-space-experiments.md) and
[media experiment details](130-media-weight-experiments.md) for exact limitations,
native evidence locations and rejected attempts.
