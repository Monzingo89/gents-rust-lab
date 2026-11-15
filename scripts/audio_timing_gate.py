"""Fail-closed timing comparison. A mean gain cannot hide a per-case regression."""
import math


def compare_timing(before, after):
    def index(cases):
        result = {}
        for case in cases:
            key = case['id']
            if not isinstance(key, str) or not key or key in result:
                raise ValueError('invalid or duplicate case identity')
            for field in ('reference_seconds', 'predicted_seconds'):
                value = case[field]
                if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
                    raise ValueError('invalid duration')
            result[key] = case
        if not result:
            raise ValueError('empty evaluation')
        return result
    left, right = index(before), index(after)
    if left.keys() != right.keys():
        raise ValueError('case identities differ')
    errors, regressions = [], []
    for key, base in left.items():
        candidate = right[key]
        if base['reference_seconds'] != candidate['reference_seconds']:
            raise ValueError('reference changed')
        reference = base['reference_seconds']
        old = abs(base['predicted_seconds']-reference)/reference
        new = abs(candidate['predicted_seconds']-reference)/reference
        errors.append((old,new))
        if new > old:
            regressions.append(key)
    old = sum(x for x,y in errors)/len(errors)
    new = sum(y for x,y in errors)/len(errors)
    return {'baseline_mean_error':old,'candidate_mean_error':new,'regressions':regressions,
        'measured_improvement':new < old and not regressions,
        'production_qualified':False}
