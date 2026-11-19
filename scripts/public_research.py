import re
import math


def redact_public_at_tokens(value):
    marker = "[PUBLIC_TOKEN_REDACTED]"
    token_pattern = re.compile(r"\S+")

    def clean_string(text):
        count = 0

        def replace_token(match):
            nonlocal count
            token = match.group(0)
            if "@" in token:
                count += 1
                return marker
            return token

        return token_pattern.sub(replace_token, text), count

    def clean_value(item):
        if isinstance(item, dict):
            cleaned = {}
            total = 0
            for key, child in item.items():
                if not isinstance(key, str) or "@" in key:
                    raise ValueError("dict keys must be strings without at-signs")
                cleaned_child, child_count = clean_value(child)
                cleaned[key] = cleaned_child
                total += child_count
            return cleaned, total

        if isinstance(item, list):
            cleaned = []
            total = 0
            for child in item:
                cleaned_child, child_count = clean_value(child)
                cleaned.append(cleaned_child)
                total += child_count
            return cleaned, total

        if isinstance(item, str):
            return clean_string(item)

        if isinstance(item, bool):
            return item, 0

        if item is None:
            return None, 0

        if isinstance(item, int):
            return item, 0

        if isinstance(item, float):
            if not math.isfinite(item):
                raise ValueError("float values must be finite")
            return item, 0

        raise TypeError("unsupported JSON value type")

    return clean_value(value)
