#!/usr/bin/env python3
"""Read-only evidence index. Never grants admission or combines unlike benchmarks."""

import hashlib
import json
from pathlib import Path

from model_improvement_inventory import inventory

ROOT = Path(__file__).resolve().parents[1]
TEXT = [
    ("weight-experiment", "simple extraction"),
    ("weight-event-fold", "event folding"),
    ("weight-diverse-events", "diverse events"),
    ("weight-four-layer", "four-layer event reconciliation"),
    ("weight-output-intent", "strict output-intent formatting"),
]


def classify(before, after, total, regressions):
    if (
        any(type(n) is not int for n in (before, after, total, regressions))
        or not 0 <= before <= total
        or not 0 <= after <= total
        or total <= 0
        or not 0 <= regressions <= total
    ):
        raise ValueError("invalid benchmark counts")
    if regressions:
        return "rejected_regression"
    if after > before:
        return "narrow_measured_gain_unqualified"
    return "no_measured_gain"


def report(root=ROOT):
    result = inventory(root)
    evidence = root / "docs/evidence"
    sources = {}
    experiments = []

    def read(name):
        path = evidence / (name + "-2026-11-14.json")
        raw = path.read_bytes()
        sources[str(path.relative_to(root))] = hashlib.sha256(raw).hexdigest()
        return json.loads(raw)

    for name, task in TEXT:
        d = read(name)
        c = d["task_comparison"]
        experiments.append(
            {
                "model": "gemma-4-E4B-it",
                "task": task,
                "source": name + "-2026-11-14.json",
                "before": c["before_passed"],
                "after": c["after_passed"],
                "cases": c["total"],
                "regressions": c["regressions"],
                "status": classify(
                    c["before_passed"], c["after_passed"], c["total"], c["regressions"]
                ),
            }
        )
    diagnosis = read("weight-output-intent-diagnosis")
    experiments[-1]["semantic_diagnostic"] = {
        stage: diagnosis["stages"][stage]["diagnostic_label_set_correct"]
        for stage in ("baseline", "candidate")
    }
    experiments[-1]["limitation"] = (
        "Strict format gain; diagnostic label accuracy did not improve. Diagnostic is post hoc, not a production parser."
    )
    d = read("image-count-weight-trial")
    experiments.append(
        {
            "model": "flux2-klein-4b-4bit",
            "task": "four same-concept square-counting cases",
            "before": d["before_passed"],
            "after": d["after_passed"],
            "cases": len(d["cases"]),
            "status": classify(
                d["before_passed"],
                d["after_passed"],
                len(d["cases"]),
                len(d["regressions"]),
            ),
        }
    )
    d = read("audio-timing-weight-trial")
    strict = read("audio-weight-interpolation")
    experiments.append(
        {
            "model": "piper-ljspeech",
            "task": "reference speech duration",
            "results": d["results"],
            "status": "rejected_regression"
            if any(
                g["regressions"]
                for g in strict["original_candidate_strict_gate"].values()
            )
            else "unqualified",
            "limitation": "Likely pretraining overlap. Duration error is not intelligibility or naturalness.",
        }
    )
    experiments.append(
        {
            "model": "piper-ljspeech",
            "task": "duration weight interpolation validation search",
            "trials": strict["trials"],
            "status": "rejected_regression"
            if strict["selected_scale"] is None
            else "unqualified",
        }
    )
    for model in result["models"]:
        model["experiments"] = [
            e for e in experiments if e["model"] == model["directory"]
        ]
        model["improvement_status"] = (
            "experimental_unqualified" if model["experiments"] else "not_evaluated"
        )
        model["production_improvement_qualified"] = False
    result.pop("weights_modified", None)
    result.update(
        report_generation_modifies_weights=False,
        evidence_sha256=sources,
        experiments=experiments,
        interpretation="Index of persisted experiment evidence, not a fresh native execution or admission decision. Scores from different datasets must not be averaged.",
        qualified_production_improvements=0,
        capability_gaps=[
            "Other language models lack measured before/after results.",
            "Vision-language image understanding has no qualified weight experiment.",
            "Image counting adapter showed no gain.",
            "Speech timing candidates regressed on individual cases; general speech quality unmeasured.",
            "No registered video generation backend or video weight experiment.",
            "No universal task capability demonstrated.",
        ],
    )
    return result


if __name__ == "__main__":
    print(json.dumps(report(), indent=2))
