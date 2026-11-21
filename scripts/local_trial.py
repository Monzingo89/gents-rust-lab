"""Owned local probe construction, instance readiness and bounded group cleanup.

Control-plane readiness is separate from model/tool/task readiness. This factory
never registers a company, grants capabilities or starts inference itself.
"""

import json
import os
import signal
import socket
import subprocess
import threading
import time
import urllib.error
import uuid
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

from experiment_loop import LoopbackAPI


def port():
    """Legacy live-probe import contract; readiness still validates ownership."""
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


@dataclass
class LocalTrial:
    api: LoopbackAPI
    child: subprocess.Popen
    startup_id: str
    shutdown_forced: bool | None = None


def wait_ready(child, api, startup_id, timeout):
    if signal.getitimer(signal.ITIMER_REAL) != (0.0, 0.0):
        raise RuntimeError("Owned readiness cannot replace an active process timer")
    previous_alarm = signal.getsignal(signal.SIGALRM)

    def expired(signum, frame):
        raise TimeoutError("Owned trial harness readiness deadline exceeded")

    deadline = time.monotonic() + timeout
    signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, timeout)
    try:
        while True:
            if child.poll() is not None:
                raise RuntimeError(
                    "Owned trial harness stopped before readiness; inspect its log"
                )
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("Owned trial harness readiness deadline exceeded")
            try:
                config = api("/api/config", timeout=min(1, remaining))
            except urllib.error.HTTPError:
                raise ValueError(
                    "Trial address is occupied by an incompatible service"
                ) from None
            except OSError:
                time.sleep(min(0.05, max(0, deadline - time.monotonic())))
                continue
            if not isinstance(config, dict) or not (
                config.get("auth") == "local"
                and config.get("local_only") is True
                and config.get("company_mode") is True
                and config.get("company_tools") is True
            ):
                raise ValueError(
                    "Trial readiness requires local authentication, local-only inference and company tools"
                )
            if config.get("startup_id") != startup_id:
                raise ValueError("Trial listener is not the owned process instance")
            if time.monotonic() >= deadline:
                raise TimeoutError("Owned trial harness readiness deadline exceeded")
            if child.poll() is not None:
                raise RuntimeError("Owned trial harness exited during readiness")
            return config
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_alarm)


def active_group(pid):
    """Independent liveness observation; permission failure is not absence."""
    result = subprocess.run(
        ["/bin/ps", "-axo", "pgid=,stat="],
        capture_output=True,
        text=True,
        timeout=1,
        env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"},
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError("Could not reconcile owned process-group liveness")
    for line in result.stdout.splitlines():
        fields = line.split()
        if len(fields) != 2 or not fields[0].isdigit():
            raise RuntimeError("Malformed process-group observation")
        if int(fields[0]) == pid and not fields[1].startswith("Z"):
            return True
    return False


def stop_group(child):
    def send(sig):
        try:
            os.killpg(child.pid, sig)
            return True
        except ProcessLookupError:
            return False
        except PermissionError:
            alive = active_group(child.pid)
            if sig == 0 or not alive:
                return alive
            raise  # An active group that cannot be signalled is not cleaned up.

    send(signal.SIGTERM)
    deadline = time.monotonic() + 2
    while True:
        child.poll()  # Reap the leader even when descendants still hold the group.
        if not send(0):
            child.wait(timeout=1)
            return False
        if time.monotonic() >= deadline:
            send(signal.SIGKILL)
            child.wait(timeout=1)
            return True
        time.sleep(0.025)


@contextmanager
def local_trial(root, config, output, *, http_port=None, readiness_seconds=20):
    if os.name != "posix" or threading.current_thread() is not threading.main_thread():
        raise ValueError("Owned trial lifecycle requires a POSIX main thread")
    if not 0 < readiness_seconds <= 60:
        raise ValueError("Readiness deadline must be positive and at most 60 seconds")
    root, config, output = (
        Path(root).resolve(),
        Path(config).resolve(),
        Path(output).resolve(),
    )
    # Hold both reservations together to keep HTTP and model ports distinct.
    # Another process can still bind after release; instance readiness detects it.
    with socket.socket() as first, socket.socket() as second:
        first.bind(("127.0.0.1", 0))
        second.bind(("127.0.0.1", 0))
        one, two = first.getsockname()[1], second.getsockname()[1]
        http_port = one if http_port is None else http_port
        model_port = two if http_port != two else one
    startup_id = str(uuid.uuid4())
    api = LoopbackAPI(f"http://127.0.0.1:{http_port}")
    env = {
        k: v
        for k, v in os.environ.items()
        if k in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}
    }
    env.update(
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{http_port}",
        CONFIG=str(config),
        RUN_STORE=str(output / "runs.sqlite3"),
        LOCAL_MODEL_PORT=str(model_port),
        GENTS_STARTUP_ID=startup_id,
        GENTS_LOCAL_ONLY="1",
        GENTS_OFFICE_ACTIVITY="0",
        GENTS_DISABLE_SCHEDULER="1",
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    previous = signal.getsignal(signal.SIGTERM)
    previous_interrupt = signal.getsignal(signal.SIGINT)

    def terminate(signum, frame):
        raise SystemExit(128 + signum)

    with (output / "harness.log").open("wb") as log:
        child = subprocess.Popen(
            [str(root / "target/debug/gents-harness")],
            cwd=root,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        trial = LocalTrial(api, child, startup_id)
        signal.signal(signal.SIGTERM, terminate)
        started = time.monotonic()
        ready = False
        failure = None
        try:
            wait_ready(child, api, startup_id, readiness_seconds)
            ready = True
            yield trial
        except BaseException as error:
            failure = type(error).__name__
            raise
        finally:
            # Outer supervisors allow five seconds of TERM cleanup. Keep the
            # owned group's TERM/KILL/reap sequence within that grace period.
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            signal.signal(signal.SIGINT, signal.SIG_IGN)
            cleanup_failure = None
            try:
                trial.shutdown_forced = stop_group(child)
            except BaseException as error:
                cleanup_failure = type(error).__name__
                raise
            finally:
                signal.signal(signal.SIGTERM, previous)
                signal.signal(signal.SIGINT, previous_interrupt)
                record = {
                    "schema_version": 1,
                    "scope": "owned probe control plane; not model/tool/task readiness",
                    "startup_id": startup_id,
                    "control_plane_ready": ready,
                    "child_exit_code": child.returncode,
                    "shutdown_forced": trial.shutdown_forced,
                    "failure_type": failure,
                    "cleanup_failure_type": cleanup_failure,
                    "elapsed_seconds": time.monotonic() - started,
                }
                (output / "lifecycle.json").write_text(
                    json.dumps(record, indent=2) + "\n"
                )
