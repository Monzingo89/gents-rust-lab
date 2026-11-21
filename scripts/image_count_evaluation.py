"""Deterministic colored connected-component checks; not a learned quality judge."""


def measure(image, case, criteria):
    if image.size != (256, 256) or case["color"] != "green":
        raise ValueError("fixed green 256px counting benchmark required")
    pixels = list(image.convert("RGB").get_flattened_data())
    mask = {
        i
        for i, (r, g, b) in enumerate(pixels)
        if g > 70 and g > 1.4 * r and g > 1.4 * b
    }
    foreground = len(mask)
    components = []
    while mask:
        origin = mask.pop()
        pending = [origin]
        part = [origin]
        while pending:
            i = pending.pop()
            x = i % 256
            y = i // 256
            neighbors = []
            if x:
                neighbors.append(i - 1)
            if x < 255:
                neighbors.append(i + 1)
            if y:
                neighbors.append(i - 256)
            if y < 255:
                neighbors.append(i + 256)
            for n in neighbors:
                if n in mask:
                    mask.remove(n)
                    pending.append(n)
                    part.append(n)
        if len(part) >= criteria["minimum_component_pixels"]:
            xs = [i % 256 for i in part]
            ys = [i // 256 for i in part]
            w = max(xs) - min(xs) + 1
            h = max(ys) - min(ys) + 1
            components.append(
                {"pixels": len(part), "fill": len(part) / (w * h), "aspect": w / h}
            )
    white = sum(min(p) > 235 for p in pixels) / len(pixels)
    low, high = criteria["component_aspect_ratio"]
    checks = {
        "exact_count": len(components) == case["count"],
        "foreground_area": foreground / len(pixels)
        >= criteria["minimum_foreground_fraction"],
        "white_background": white >= criteria["minimum_white_fraction"],
        "square_components": bool(components)
        and all(
            c["fill"] >= criteria["minimum_component_fill"]
            and low <= c["aspect"] <= high
            for c in components
        ),
    }
    return {
        "passed": all(checks.values()),
        "requested_count": case["count"],
        "observed_count": len(components),
        "checks": checks,
        "components": components,
        "white_fraction": white,
    }
