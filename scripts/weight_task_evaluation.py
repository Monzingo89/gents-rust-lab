"""Bounded task evaluation, separate from training loss and promotion authority."""
import hashlib
import json
import time


def score(output, expected):
    """Exact text, or strict JSON equality (no bool/number coercion)."""
    if output.strip() == expected.strip():
        return True
    def strict(value):
        def pairs(items):
            result = {}
            for key, item in items:
                if key in result:
                    raise ValueError("duplicate JSON key")
                result[key] = item
            return result
        def invalid(_):
            raise ValueError("nonfinite JSON")
        parsed = json.loads(value, object_pairs_hook=pairs, parse_constant=invalid)
        return json.dumps(parsed, sort_keys=True, ensure_ascii=False)
    try:
        return strict(output) == strict(expected)
    except (ValueError, TypeError):
        return False


def evaluate_tasks(rows, infer):
    if not 1 <= len(rows) <= 64:
        raise ValueError("task evaluation requires 1..64 frozen cases")
    results = []
    for row in rows:
        messages = row["messages"]
        if messages[-1]["role"] != "assistant":
            raise ValueError("missing expected answer")
        prompt = messages[:-1]
        expected = messages[-1]["content"]
        started = time.monotonic()
        output = infer(prompt)
        results.append({"prompt_sha256": hashlib.sha256(json.dumps(prompt, sort_keys=True).encode()).hexdigest(),
                        "output": output, "expected": expected,
                        "passed": score(output, expected),
                        "elapsed_seconds": round(time.monotonic() - started, 3)})
    return {"cases": results, "passed": sum(r["passed"] for r in results), "total": len(results),
            "criterion": "exact text after trimming or strict canonical JSON equality",
            "provider_cost_usd": 0, "electricity_cost": "unmeasured"}


def compare(before, after):
    if [r["prompt_sha256"] for r in before["cases"]] != [r["prompt_sha256"] for r in after["cases"]]:
        raise ValueError("comparison cases changed")
    regressions = sum(a["passed"] and not b["passed"] for a, b in zip(before["cases"], after["cases"]))
    return {"before_passed": before["passed"], "after_passed": after["passed"],
            "total": before["total"], "regressions": regressions,
            "measured_improvement": after["passed"] > before["passed"] and regressions == 0,
            "promotion_authorized": False,
            "scope": "Only these frozen cases; no general capability or statistical significance claim"}
