#!/usr/bin/env python3
"""Daily offline model integrity/format checks; no vulnerability-free certification."""
import argparse
import json
import os
from pathlib import Path
import stat
import time
import uuid
from model_security_inspection import directory, ordinary_file, inspect

ROOT = Path(__file__).resolve().parents[1]
DAY = 24 * 3600


def write_report(fd, name, value):
    temporary = "scan-" + uuid.uuid4().hex + ".tmp"
    out = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=fd)
    try:
        with os.fdopen(out, "w") as stream:
            json.dump(value, stream, allow_nan=False)
            stream.flush()
            os.fsync(stream.fileno())
        os.rename(temporary, name, src_dir_fd=fd, dst_dir_fd=fd)
        os.fsync(fd)
    finally:
        try:
            os.unlink(temporary, dir_fd=fd)
        except FileNotFoundError:
            pass


def daily(root=ROOT, force=False):
    import fcntl
    folder = directory(root / "data/model-security", create=True)
    lock = os.open("scan.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600, dir_fd=folder)
    try:
        if not stat.S_ISREG(os.fstat(lock).st_mode) or os.fstat(lock).st_nlink != 1:
            raise ValueError("invalid scan lock")
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return {"status":"SCAN_ALREADY_RUNNING"}
        try:
            with ordinary_file(folder, "latest.json") as stream:
                raw = stream.read(16 * 1024 * 1024 + 1)
                if len(raw) > 16 * 1024 * 1024:
                    raise ValueError("scan report exceeds bound")
                previous = json.loads(raw)
            age = time.time() - previous["completed_at_seconds"]
            if not force and 0 <= age < DAY:
                return {"status":"NOT_DUE","latest_status":previous["status"]}
        except FileNotFoundError:
            pass
        roster = json.loads((root / "config/enterprise-model-roster.json").read_text())
        pins = {model["path"]:model["sha256"] for model in roster["models"]}
        started = time.time()
        result = inspect(root,pins,lambda path:print(json.dumps({"status":"SCANNING","path":path}),flush=True))
        result.update(started_at_seconds=started,completed_at_seconds=time.time(),scan_id=uuid.uuid4().hex)
        write_report(folder,result["scan_id"]+".json",result)
        write_report(folder,"latest.json",result)
        return {"status":result["status"],"scan_id":result["scan_id"],"artifacts":len(result["artifacts"]),"findings":len(result["findings"]),"provider_cost_usd":0}
    finally:
        os.close(lock)
        os.close(folder)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force",action="store_true")
    parser.add_argument("--watch",action="store_true",help="Check hourly while this local process remains running")
    args = parser.parse_args()
    while True:
        try:
            print(json.dumps(daily(force=args.force)),flush=True)
        except (OSError, ValueError, RuntimeError, KeyError) as error:
            print(json.dumps({"status":"SCAN_ERROR","error_type":type(error).__name__}),flush=True)
            if not args.watch:
                raise SystemExit(1) from None
        if not args.watch:
            break
        args.force = False
        for _ in range(60):
            time.sleep(60)


if __name__ == "__main__":
    main()
