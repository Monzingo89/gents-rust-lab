from __future__ import annotations

import json
import math
import os
import tempfile
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple


def _is_finite_number(value: Any) -> bool:
    if isinstance(value, bool):
        return False
    if isinstance(value, (int, float)):
        return math.isfinite(value)
    return False


def _validate_evidence_name(name: Any) -> str:
    if not isinstance(name, str):
        raise TypeError("evidence name must be a string")
    if not name or name in {".", ".."}:
        raise ValueError("evidence name must not be empty or a dot name")
    if "\x00" in name or "/" in name or "\\" in name:
        raise ValueError("evidence name must not contain separators or traversal")
    if name.startswith("/") or name.startswith("\\"):
        raise ValueError("evidence name must not be absolute")
    return name


def _canonical_json_bytes(value: Dict[str, Any]) -> bytes:
    if not isinstance(value, dict):
        raise TypeError("evidence value must be a dict")
    try:
        encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise ValueError("evidence value must be JSON-serializable and finite") from exc
    return encoded


class EvidenceStore:
    def __init__(self, directory: Path) -> None:
        if not isinstance(directory, Path):
            raise TypeError("directory must be a pathlib.Path")
        self._directory = directory

    def _target(self, name: str) -> Path:
        return self._directory / name

    def _ensure_directory(self) -> None:
        if self._directory.is_symlink():
            raise ValueError("evidence directory must not be a symlink")
        if not self._directory.exists():
            self._directory.mkdir(parents=True)
        if not self._directory.is_dir():
            raise ValueError("evidence directory must be a directory")

    def write_if_changed(self, name: str, value: Dict[str, Any]) -> bool:
        name = _validate_evidence_name(name)
        target = self._target(name)
        self._ensure_directory()
        if target.is_symlink():
            raise ValueError("evidence target must not be a symlink")
        new_bytes = _canonical_json_bytes(value)
        if target.exists():
            if target.is_symlink():
                raise ValueError("evidence target must not be a symlink")
            existing_bytes = target.read_bytes()
            if existing_bytes == new_bytes:
                return False
        fd: int
        tmp_path: Path
        try:
            fd, tmp_name = tempfile.mkstemp(dir=self._directory, prefix=f".{name}.", suffix=".tmp")
            tmp_path = Path(tmp_name)
            with os.fdopen(fd, "wb") as handle:
                handle.write(new_bytes)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(tmp_path, target)
        except BaseException:
            try:
                if "tmp_path" in locals() and tmp_path.exists():
                    tmp_path.unlink()
            except OSError:
                pass
            raise
        return True


def _validate_run_id(run_id: Any) -> str:
    if not isinstance(run_id, str) or not run_id:
        raise ValueError("run_id must be a non-empty safe path segment")
    for char in run_id:
        if not (char.isalnum() or char in "-_"):
            raise ValueError("run_id must contain only alphanumeric characters, hyphens, or underscores")
    return run_id


def _validate_positive_finite(value: Any, label: str) -> float:
    if isinstance(value, bool) or not _is_finite_number(value):
        raise ValueError(f"{label} must be a finite number")
    number = float(value)
    if number <= 0:
        raise ValueError(f"{label} must be positive")
    return number


def _validate_nonnegative_finite(value: Any, label: str) -> float:
    if isinstance(value, bool) or not _is_finite_number(value):
        raise ValueError(f"{label} must be a finite number")
    number = float(value)
    if number < 0:
        raise ValueError(f"{label} must be non-negative")
    return number


def _as_dict(value: Any, label: str) -> Dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a dict")
    return value


def _get_dict(value: Any, key: str, label: str) -> Dict[str, Any]:
    if not isinstance(value, dict) or key not in value:
        raise ValueError(f"{label} is missing")
    return _as_dict(value[key], f"{label}.{key}")


def _get_list(value: Any, key: str, label: str) -> List[Any]:
    if not isinstance(value, dict) or key not in value:
        raise ValueError(f"{label} is missing")
    item = value[key]
    if not isinstance(item, list):
        raise ValueError(f"{label}.{key} must be a list")
    return item


def _get_string(value: Any, key: str, label: str) -> str:
    if not isinstance(value, dict) or key not in value:
        raise ValueError(f"{label} is missing")
    item = value[key]
    if not isinstance(item, str):
        raise ValueError(f"{label}.{key} must be a string")
    return item


def _get_cost(value: Any, key: str, label: str) -> float:
    if not isinstance(value, dict) or key not in value:
        raise ValueError(f"{label} is missing")
    item = value[key]
    if isinstance(item, bool) or not _is_finite_number(item):
        raise ValueError(f"{label}.{key} must be a finite number")
    number = float(item)
    if number != 0:
        raise ValueError(f"{label}.{key} must be exactly zero")
    return number


def _extract_reply(event: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    if "reply" not in event:
        return None
    reply = event["reply"]
    if reply is None:
        return None
    return _as_dict(reply, "event.reply")


def _extract_cost(reply: Dict[str, Any]) -> float:
    _as_dict(reply, "reply")
    usage = reply.get("usage")
    if usage is None:
        raise ValueError("Missing usage key")
    _as_dict(usage, "usage")
    return _get_cost(usage, "cost_usd", "usage")


def _summarize_snapshot(snapshot: Dict[str, Any]) -> Tuple[str, int, float]:
    runtime_status = _get_string(snapshot, "status", "snapshot")
    events = _get_list(snapshot, "events", "snapshot")
    model_calls = 0
    provider_cost_usd = 0.0
    for event in events:
        event = _as_dict(event, "snapshot event")
        reply = _extract_reply(event)
        if reply is None:
            continue
        model_calls += 1
        provider_cost_usd += _extract_cost(reply)
    if not _is_finite_number(provider_cost_usd):
        raise ValueError("provider_cost_usd must be finite")
    return runtime_status, model_calls, float(provider_cost_usd)


def _attention_is_unanswered(attention: Dict[str, Any]) -> bool:
    if attention is None:
        return False
    if not isinstance(attention, dict):
        raise ValueError("attention must be a dict")
    if "attention" not in attention:
        raise ValueError("attention.attention is missing")
    nested = attention["attention"]
    if nested is None:
        return False
    if not isinstance(nested, dict):
        raise ValueError("attention.attention must be a dict")
    if "answer" not in nested:
        raise ValueError("attention.attention.answer is missing")
    return nested["answer"] is None


def observe_run(
    api: Callable[[str], Dict[str, Any]],
    run_id: str,
    evidence_dir: Path,
    *,
    clock: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
    timeout_seconds: float = 1800,
    poll_seconds: float = 3,
) -> Dict[str, Any]:
    run_id = _validate_run_id(run_id)
    timeout_seconds = _validate_nonnegative_finite(timeout_seconds, "timeout_seconds")
    poll_seconds = _validate_positive_finite(poll_seconds, "poll_seconds")
    if not isinstance(evidence_dir, Path):
        raise TypeError("evidence_dir must be a pathlib.Path")
    if not callable(api):
        raise TypeError("api must be callable")

    store = EvidenceStore(evidence_dir)
    run_path = f"/api/runs/{run_id}"
    attention_path = f"/api/runs/{run_id}/attention"
    started_at = float(clock())
    deadline = started_at + timeout_seconds
    runtime_status = "unknown"
    model_calls = 0
    provider_cost_usd = 0.0

    while True:
        snapshot = _as_dict(api(run_path), "run snapshot")
        runtime_status, model_calls, provider_cost_usd = _summarize_snapshot(snapshot)
        store.write_if_changed("observed-run.json", snapshot)

        attention = _as_dict(api(attention_path), "attention")
        store.write_if_changed("attention.json", attention)

        # If the runtime_status is not in the active set {running, queued}, return immediately
        if runtime_status not in {"running", "queued"}:
            status = runtime_status
        elif _attention_is_unanswered(attention):
            status = "attention"
        else:
            now = float(clock())
            if now >= deadline:
                status = "observation_expired"
            else:
                sleep(max(0.0, min(poll_seconds, deadline - now)))
                continue

        elapsed_seconds = max(0.0, float(clock()) - started_at)
        report = {
            "status": status,
            "runtime_status": runtime_status,
            "elapsed_seconds": elapsed_seconds,
            "model_calls": model_calls,
            "provider_cost_usd": provider_cost_usd,
        }
        store.write_if_changed("report.json", report)
        return report
