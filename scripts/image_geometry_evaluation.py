"""Narrow procedural image criteria. Not a perceptual image-quality judge."""

import math


def measure(image, color, criteria):
    if image.size != (256, 256) or color not in {"red", "blue", "green", "purple"}:
        raise ValueError("fixed 256px RGB geometry fixture required")
    foreground = []
    correct = white = 0
    pixels = image.convert("RGB")
    for i, (r, g, b) in enumerate(pixels.get_flattened_data()):
        white += int(min(r, g, b) > 235)
        if min(r, g, b) >= 230:
            continue
        foreground.append((i % 256, i // 256))
        matches = {
            "red": r > 100 and r > 1.5 * g and r > 1.5 * b,
            "blue": b > 100 and b > 1.5 * r and b > 1.5 * g,
            "green": g > 80 and g > 1.5 * r and g > 1.5 * b,
            "purple": r > 80 and b > 80 and r > 1.4 * g and b > 1.4 * g,
        }
        correct += int(matches[color])
    if not foreground:
        return {"passed": False, "reason": "no foreground"}
    xs, ys = zip(*foreground)
    count = len(xs)
    width = max(xs) - min(xs) + 1
    height = max(ys) - min(ys) + 1
    values = {
        "foreground_area_fraction": count / (256 * 256),
        "centroid_distance_from_center": math.hypot(
            sum(xs) / count - 127.5, sum(ys) / count - 127.5
        )
        / 256,
        "foreground_bbox_aspect_ratio": width / height,
        "foreground_fill_fraction": count / (width * height),
        "dominant_requested_color_fraction": correct / count,
        "white_background_fraction": white / (256 * 256),
    }
    checks = {}
    for key, bound in criteria.items():
        if key.endswith("_min"):
            checks[key] = values[key[:-4]] >= bound
        elif key.endswith("_max"):
            checks[key] = values[key[:-4]] <= bound
        else:
            checks[key] = bound[0] <= values[key] <= bound[1]
    return {"passed": all(checks.values()), "checks": checks, "measurements": values}
