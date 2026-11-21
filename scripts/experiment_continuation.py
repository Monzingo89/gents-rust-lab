"""Owner-frozen continuation rules and bounded untrusted repair references."""

import experiment_evaluation as evaluation


def validate(trial, earlier):
    if "after" not in trial:
        return
    after = trial["after"]
    if (
        not isinstance(after, dict)
        or set(after) != {"experiment", "outcome"}
        or not isinstance(after["experiment"], str)
        or after["outcome"] not in ("accepted", "rejected")
        or after["experiment"] not in earlier
    ):
        raise ValueError(
            "continuation requires an earlier experiment and exact accepted/rejected condition"
        )
    parent = earlier[after["experiment"]]
    left, right = parent.get("evaluation", {}), trial.get("evaluation", {})
    if (
        left.get("kind") != "rust_acceptance"
        or right.get("kind") != "rust_acceptance"
        or left.get("tests_sha256") != right.get("tests_sha256")
        or trial["task"] != parent["task"]
        or trial["criteria"] != parent["criteria"]
        or "candidate_workflow" not in trial
        or "candidate_workflow" not in parent
    ):
        raise ValueError(
            "continuation must preserve the frozen source task, criteria and Rust acceptance hash"
        )


def prepare(
    root,
    company,
    parent_run,
    parent_trial,
    parent_result,
    condition,
    prior_sources,
    result_hash,
    evaluation_hash,
    read_file,
):
    checked = parent_result.get("evaluation", {})
    if (
        parent_result.get("runtime_status") != "candidate_submitted"
        or checked.get("kind") != "rust_acceptance"
        or type(checked.get("passed")) is not bool
        or "native_result" not in checked
    ):
        return {
            "decision": "skip",
            "reason": "predecessor has no independent acceptance outcome",
        }
    outcome = "accepted" if checked["passed"] else "rejected"
    if parent_result.get("independently_accepted") is not checked["passed"]:
        raise ValueError("predecessor acceptance flags disagree")
    if outcome != condition:
        return {
            "decision": "skip",
            "reason": "predecessor independent outcome does not match frozen condition",
        }
    source, custody = evaluation.custody(
        root, company, parent_run, parent_trial, read_file
    )
    spec, native = parent_trial["evaluation"], checked["native_result"]
    if (
        custody != checked["custody"]
        or checked["runtime"] != spec["runtime"]
        or checked["tests_sha256"] != spec["tests_sha256"]
        or native.get("tests_sha256") != spec["tests_sha256"]
        or native.get("source_sha256") != custody["source_sha256"]
        or native.get("passed") is not checked["passed"]
        or native.get("stage")
        not in ("admission", "compile_candidate", "compile_acceptance", "acceptance")
    ):
        raise ValueError("predecessor evaluation differs from current frozen custody")
    for key in ("adapter", "compiler"):
        if (
            evaluation.executable_pin(spec["runtime"][key]["path"])
            != spec["runtime"][key]
        ):
            raise ValueError("predecessor evaluation runtime changed")
    if custody["source_sha256"] in prior_sources:
        return {
            "decision": "skip",
            "reason": "predecessor repeated an earlier source; no new revision to continue",
        }
    diagnostics = native["diagnostics"]
    excerpt = (
        diagnostics
        if isinstance(diagnostics, str)
        else "\n".join(
            value
            for value in (diagnostics.get("stderr", ""), diagnostics.get("stdout", ""))
            if value
        )
    )
    excerpt = excerpt.encode()[:1200].decode("utf-8", errors="ignore")
    feedback = {
        "version": 1,
        "prior_run": parent_run,
        "source_path": parent_trial["candidate_workflow"]["path"],
        "source_sha256": custody["source_sha256"],
        "source": source,
        "tests_sha256": spec["tests_sha256"],
        "evaluation_sha256": evaluation_hash,
        "outcome": outcome,
        "stage": native["stage"],
        "diagnostics_excerpt": excerpt,
    }
    return {
        "decision": "continue",
        "feedback": feedback,
        "predecessor_result_sha256": result_hash,
    }
