"""Owned bounded experiments; model output cannot supply optimizer configuration."""
PROFILES = {
    "conservative-v1": {"layers":1,"rank":4,"scale":8.0,"learning_rate":1e-5},
    "four-layer-v1": {"layers":4,"rank":8,"scale":16.0,"learning_rate":1e-5},
}


def profile(name="conservative-v1"):
    if not isinstance(name,str) or name not in PROFILES:
        raise ValueError("unknown bounded training profile")
    return dict(PROFILES[name])
