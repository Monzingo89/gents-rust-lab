"""Compile owner-enabled research defaults for fresh, not-yet-approved jobs."""

import hashlib
import json

RESEARCH_TOOLS = frozenset(
    {
        "web_search",
        "web_read",
        "source_read",
        "skill_list",
        "skill_read",
        "memory_search",
        "memory_append",
    }
)


def fresh_access(profile_path, actors, primary, primary_tools):
    """Return a new policy access map; never load, replace or migrate a policy."""
    raw = profile_path.read_bytes()
    profile = json.loads(raw)
    if profile.get("version") != 1 or type(profile.get("enabled")) is not bool:
        raise ValueError("Invalid owner research profile")
    requested = profile.get("tools")
    if not isinstance(requested, list) or any(type(t) is not str for t in requested):
        raise ValueError("Invalid research tools")
    if not set(requested) <= RESEARCH_TOOLS:
        raise ValueError(
            "Research defaults cannot add execution or external-write tools"
        )
    if not actors or len(actors) != len(set(actors)) or primary not in actors:
        raise ValueError("Research requires exact unique configured actors")
    if any(type(actor) is not str or not actor.strip() for actor in actors):
        raise ValueError("Invalid actor identity")
    research = set(requested) if profile["enabled"] else set()
    access = {
        actor: {
            "tools": sorted(
                research | (set(primary_tools) if actor == primary else set())
            ),
            "skills": [],
            "memory_from": [],
        }
        for actor in actors
    }
    return access, {
        "enabled": profile["enabled"],
        "profile_sha256": hashlib.sha256(raw).hexdigest(),
        "actors": list(actors),
        "research_tools": sorted(research),
        "scope": "Fresh owner-installed policy only; task and phase checks still apply",
    }
