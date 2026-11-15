"""Private bookkeeping DB-API subset backed by owned, patched Rust SQLite.

Never imports the system sqlite3 driver, reconnects or replays requests. This is
not an agent tool or general DB-API implementation. Unsupported modes fail closed.
"""
import json
import math
import os
from pathlib import Path
import queue
import stat
import subprocess
import threading
from urllib.parse import urlsplit, parse_qs, unquote, urlencode
from urllib.request import url2pathname

MAX_WIRE = 8 * 1024 * 1024
MIN_SQLITE = (3, 54, 0)

class Error(Exception): pass
class DatabaseError(Error): pass
class IntegrityError(DatabaseError): pass
class OperationalError(DatabaseError): pass
class ProgrammingError(DatabaseError): pass
class DataError(DatabaseError): pass
class NotSupportedError(DatabaseError): pass

def _environment():
    allowed = {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}
    return {k: v for k, v in os.environ.items() if k.upper() in allowed}

def _binary():
    default = Path(__file__).resolve().parents[1] / "target/debug" / ("gents-sqlite-bridge.exe" if os.name == "nt" else "gents-sqlite-bridge")
    path = Path(os.environ.get("GENTS_SQLITE_BRIDGE", default))
    try:
        info = path.lstat()
    except OSError:
        raise OperationalError("Owned SQLite bridge missing; run cargo build --workspace --locked") from None
    if not stat.S_ISREG(info.st_mode) or (info.st_mode & 0o022) or (hasattr(os, "getuid") and info.st_uid != os.getuid()):
        raise OperationalError("Owned SQLite bridge must be an owner-controlled ordinary executable")
    return path

def _reader(stream, responses):
    try:
        while True:
            value = stream.readline(MAX_WIRE + 1)
            if not value:
                break
            if len(value) > MAX_WIRE or not value.endswith(b"\n"):
                break
            responses.put(value)
    finally:
        responses.put(None)
        stream.close()

def _parameter(value):
    if value is None or isinstance(value, str) or type(value) is bool:
        return value
    if type(value) is int:
        if not -(2**63) <= value < 2**63:
            raise OverflowError("SQLite integer outside signed 64-bit range")
        return value
    if type(value) is float and math.isfinite(value):
        return value
    if isinstance(value, (bytes, bytearray, memoryview)):
        return {"__blob_hex": bytes(value).hex()}
    raise NotSupportedError("Only scalar and binary positional SQLite parameters are supported")

def _cell(value):
    if isinstance(value, dict) and set(value) == {"__blob_hex"}:
        return bytes.fromhex(value["__blob_hex"])
    if value is None or type(value) in (str, int, float):
        return value
    raise DatabaseError("Invalid owned SQLite result")

class Row:
    def __init__(self, cursor, values):
        self._names = tuple(c[0] for c in cursor.description)
        self._values = tuple(values)
    def keys(self): return list(self._names)
    def __iter__(self): return iter(self._values)
    def __len__(self): return len(self._values)
    def __getitem__(self, key):
        if isinstance(key, str):
            key = next((i for i, name in enumerate(self._names) if name.lower() == key.lower()), None)
            if key is None: raise IndexError("No SQLite column with that name")
        return self._values[key]

class Cursor:
    def __init__(self, connection):
        self.connection = connection
        self.row_factory = connection.row_factory
        self.description = None
        self.rowcount = -1
        self.lastrowid = None
        self.arraysize = 1
        self._rows, self._offset = [], 0
        self._closed = False
    def _check(self):
        self.connection._check()
        if self._closed: raise ProgrammingError("SQLite cursor is closed")
    def execute(self, sql, parameters=()):
        self._check()
        if not isinstance(parameters, (tuple, list)):
            raise NotSupportedError("Only positional SQLite parameters are supported")
        result = self.connection._request("execute", sql=sql, params=[_parameter(v) for v in parameters])
        columns = result["columns"]
        self.description = tuple((name, None, None, None, None, None, None) for name in columns) if columns else None
        self._rows = [tuple(_cell(v) for v in row) for row in result["rows"]]
        self._offset = 0
        self.rowcount, self.lastrowid = result["rowcount"], result["lastrowid"]
        return self
    def executemany(self, sql, parameters):
        self._check()
        self.connection._request("validate_dml", sql=sql)
        previous_id = self.lastrowid
        total = 0
        for row in parameters:
            self.execute(sql, row)
            total += self.rowcount
        self.rowcount, self.lastrowid = total, previous_id
        return self
    def fetchone(self):
        self._check()
        if self._offset >= len(self._rows): return None
        row = self._rows[self._offset]
        self._offset += 1
        return self.row_factory(self, row) if self.row_factory else row
    def fetchmany(self, size=None):
        self._check()
        size = self.arraysize if size is None else size
        if type(size) is not int or size < 0: raise ValueError("nonnegative fetch size required")
        rows = []
        for _ in range(min(size, len(self._rows) - self._offset)):
            rows.append(self.fetchone())
        return rows
    def fetchall(self): return self.fetchmany(len(self._rows) - self._offset)
    def __iter__(self): return self
    def __next__(self):
        row = self.fetchone()
        if row is None: raise StopIteration
        return row
    def close(self):
        self._rows, self._offset, self._closed = [], 0, True

class Connection:
    def __init__(self, database, timeout=5.0, isolation_level="", check_same_thread=True, uri=False):
        if isolation_level not in ("", None):
            raise NotSupportedError("Only legacy deferred or explicit autocommit isolation is supported")
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or not 0 <= timeout <= 120:
            raise ValueError("SQLite busy timeout must be within 0..120 seconds")
        path = os.fspath(database)
        if not isinstance(path, str): raise NotSupportedError("SQLite path must be text")
        if path.startswith(("//", "\\\\")):
            raise NotSupportedError("Network database paths are not supported")
        if uri:
            parsed = urlsplit(path)
            options = parse_qs(parsed.query)
            if parsed.scheme != "file" or parsed.netloc not in ("", "localhost") or unquote(parsed.path).startswith(("//", "\\\\")) or set(options) - {"mode"} or options.get("mode", ["rw"])[0] not in ("ro", "rw", "rwc") or parsed.path == ":memory:":
                raise NotSupportedError("Only private file URI modes ro, rw and rwc are supported")
            database_path = Path(url2pathname(parsed.path))
            path = (database_path.parent.resolve(strict=True) / database_path.name).as_uri()
            if options: path += "?" + urlencode({k:v[0] for k,v in options.items()})
        elif path != ":memory:":
            database_path = Path(path)
            path = str(database_path.parent.resolve(strict=True) / database_path.name)
        self.row_factory = None
        self._closed, self._sequence, self._in_transaction = False, 0, False
        self._thread, self._check_same_thread = threading.get_ident(), check_same_thread
        self._lock = threading.RLock()
        self._responses = queue.Queue(maxsize=2)
        self._process = subprocess.Popen([str(_binary())], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                         stderr=subprocess.DEVNULL, env=_environment(), close_fds=True)
        threading.Thread(target=_reader, args=(self._process.stdout, self._responses), daemon=True).start()
        try:
            info = self._request("open", path=path, uri=bool(uri), timeout_ms=round(timeout * 1000), autocommit=isolation_level is None)
            if info["protocol_version"] != 1 or tuple(map(int, info["sqlite_version"].split("."))) < MIN_SQLITE:
                raise OperationalError("Owned SQLite bridge protocol/runtime is not approved")
            self.runtime_info = info
        except Exception:
            self._abort()
            raise
    def _check(self):
        if self._closed: raise ProgrammingError("SQLite connection is closed")
        if self._check_same_thread and self._thread != threading.get_ident():
            raise ProgrammingError("SQLite connection belongs to another thread")
    def _abort(self):
        self._closed = True
        process = getattr(self, "_process", None)
        if process:
            if process.poll() is None: process.kill()
            process.wait(timeout=5)
            if process.stdin:
                try: process.stdin.close()
                except OSError: pass
            # The daemon reader owns stdout and exits on the killed child's EOF.
    def _request(self, op, **arguments):
        with self._lock:
            self._check()
            self._sequence += 1
            wire = (json.dumps({"id": self._sequence, "request": {"op": op, **arguments}}, allow_nan=False) + "\n").encode()
            if len(wire) > MAX_WIRE: raise DataError("SQLite request exceeds IPC bound")
            try:
                self._process.stdin.write(wire)
                self._process.stdin.flush()
                raw = self._responses.get(timeout=65)
                if raw is None: raise EOFError
                response = json.loads(raw)
                if response.get("id") != self._sequence or type(response.get("ok")) is not bool:
                    raise ValueError("bad IPC response")
            except (OSError, EOFError, queue.Empty, ValueError):
                self._abort()
                raise OperationalError("SQLite outcome is uncertain; connection closed. Inspect state before any retry; no request was replayed.") from None
            if "in_transaction" in response:
                self._in_transaction = response["in_transaction"]
            if not response["ok"]:
                failure = response["error"]
                error = {"IntegrityError": IntegrityError, "OperationalError": OperationalError, "ProgrammingError": ProgrammingError}.get(failure["kind"], DatabaseError)(failure["message"])
                error.sqlite_errorcode = failure.get("sqlite_errorcode")
                raise error
            value = response["value"]
            if "in_transaction" in value: self._in_transaction = value["in_transaction"]
            return value
    @property
    def in_transaction(self):
        self._check()
        return self._in_transaction
    def cursor(self):
        self._check()
        return Cursor(self)
    def execute(self, sql, parameters=()): return self.cursor().execute(sql, parameters)
    def executemany(self, sql, parameters): return self.cursor().executemany(sql, parameters)
    def executescript(self, sql):
        self._request("script", sql=sql)
        return self.cursor()
    def commit(self): self._request("commit")
    def rollback(self): self._request("rollback")
    def close(self):
        if not self._closed:
            self._request("close")
            self._process.stdin.close()
            self._process.wait(timeout=5)
            self._closed = True
    def __enter__(self):
        self._check()
        return self
    def __exit__(self, kind, value, trace):
        if kind is None:
            try: self.commit()
            except Error:
                if not self._closed: self.rollback()
                raise
        elif not self._closed: self.rollback()
    def __del__(self):
        try: self._abort()
        except Exception: pass

def connect(database, timeout=5.0, isolation_level="", check_same_thread=True, uri=False):
    return Connection(database, timeout, isolation_level, check_same_thread, uri)
