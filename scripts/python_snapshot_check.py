"""Host-only Python verification experiment; never grants agent authority.

Runs a caller-reviewed stdlib unittest suite against explicit copied Python
inputs. No shell, package installation, result cache or unsandboxed fallback.
The native adapter may invoke the bounded stdin interface embedded at build
time. Its reports never establish independent company completion acceptance.
"""
import hashlib
import json
import os
import errno
from pathlib import Path, PurePosixPath
import resource
import signal
import stat
import subprocess
import sys
import sysconfig
import tempfile
import time

MAX_FILES = 32
MAX_BYTES = 256_000
MAX_OUTPUT = 16_384


def snapshot(root, paths):
    """Read only explicit regular Python files; reject links and partial inputs."""
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError('Expected a real input directory')
    root = root.resolve(strict=True)
    if not isinstance(paths, list) or not 1 <= len(paths) <= MAX_FILES:
        raise ValueError('Expected 1..32 explicit Python input paths')
    contents = {}
    total = 0
    for name in paths:
        if not isinstance(name, str):
            raise ValueError('Input path must be text')
        relative = PurePosixPath(name)
        if (str(relative) != name or relative.is_absolute()
                or any(p in ('.', '..') or p.startswith('.') for p in relative.parts)
                or relative.suffix != '.py' or name in contents):
            raise ValueError('Expected unique nonhidden relative .py paths')
        # Open every component without following links, including during races.
        descriptor = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            for part in relative.parts[:-1]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                dir_fd=descriptor)
                os.close(descriptor)
                descriptor = child
            file = os.open(relative.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                           dir_fd=descriptor)
            with os.fdopen(file, 'rb') as stream:
                before = os.fstat(stream.fileno())
                if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
                    raise ValueError('Input must be a regular unlinked file')
                content = stream.read(MAX_BYTES + 1)
                after = os.fstat(stream.fileno())
                if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                        after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                    raise ValueError('Input changed while reading')
        finally:
            os.close(descriptor)
        total += len(content)
        if total > MAX_BYTES:
            raise ValueError('Snapshot byte limit exceeded')
        contents[name] = content
    return contents


def manifest(contents):
    return {name: hashlib.sha256(body).hexdigest()
            for name, body in sorted(contents.items())}


def _profile(work, executable, runtime, scratch=None):
    quote = lambda value: json.dumps(str(value))
    profile = ('(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)'
            '(allow file-read* (literal "/") (subpath "/System/Library")'
            '(subpath "/usr/lib") (subpath "/private/var/db/dyld")'
            ' (literal "/dev/null") (literal "/dev/urandom")'
            f' (subpath {quote(work)}) (subpath {quote(runtime)}))'
            '(allow file-write* (literal "/dev/null"))'
            f'(allow process-exec (literal {quote(executable)}))'
            '(allow signal (target same-sandbox))')
    if scratch is not None:
        profile += (f'(allow file-read* (subpath {quote(scratch)}))'
                    f'(allow file-write* (subpath {quote(scratch)}))'
                    '(deny file-write-mode)')
    return profile


def _limits(file_bytes=MAX_OUTPUT + 1):
    for kind, limit in ((resource.RLIMIT_CORE, 0), (resource.RLIMIT_CPU, 3),
                        (resource.RLIMIT_FSIZE, file_bytes),
                        (resource.RLIMIT_NOFILE, 64)):
        resource.setrlimit(kind, (limit, limit))


def _scratch_usage(root, maximum):
    """Bounded descriptor-relative metadata scan; never follow scratch links."""
    total = count = 0
    stack = [os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)]
    try:
        while stack:
            descriptor = stack.pop()
            try:
                with os.scandir(descriptor) as entries:
                    for entry in entries:
                        count += 1
                        if count > 256:
                            return total, count
                        try:
                            info = os.stat(entry.name, dir_fd=descriptor, follow_symlinks=False)
                            if stat.S_ISDIR(info.st_mode):
                                stack.append(os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY
                                                     | os.O_NOFOLLOW, dir_fd=descriptor))
                            elif stat.S_ISREG(info.st_mode):
                                total += info.st_size
                        except OSError as error:
                            if error.errno not in (errno.ENOENT, errno.ELOOP, errno.ENOTDIR):
                                raise
                        if total > maximum:
                            return total, count
            finally:
                os.close(descriptor)
    finally:
        for descriptor in stack:
            os.close(descriptor)
    return total, count


def verify(root, paths, suite, suite_sha256, timeout_seconds=5, scratch_bytes=0):
    """Execute fresh copied bytes; only the caller can select/freeze acceptance.

    A result is untrusted experiment evidence, not a native company receipt.
    Python can tamper with its own unittest machinery; host review is separate.
    """
    if sys.platform != 'darwin' or not Path('/usr/bin/sandbox-exec').is_file():
        raise ValueError('macOS sandbox-exec required; no unsandboxed fallback')
    if type(timeout_seconds) not in (int, float) or not 0 < timeout_seconds <= 10:
        raise ValueError('Wall limit must be in (0,10] seconds')
    if type(scratch_bytes) is not int or not 0 <= scratch_bytes <= 8_388_608:
        raise ValueError('Scratch budget must be an integer in 0..8388608 bytes')
    contents = snapshot(root, paths)
    hashes = manifest(contents)
    if suite not in contents or hashes[suite] != suite_sha256:
        raise ValueError('Frozen suite hash mismatch')
    runtime = Path(sys.base_prefix).resolve(strict=True)
    executable = Path(sys.executable).resolve(strict=True)
    # The python.org framework bin/python is a launcher that spawns this
    # interpreter. Invoke the interpreter directly so runtime fork stays denied.
    framework_python = runtime / 'Resources/Python.app/Contents/MacOS/Python'
    if framework_python.is_file():
        executable = framework_python.resolve(strict=True)
    stdlib = Path(sysconfig.get_path('stdlib')).resolve(strict=True)
    if not executable.is_relative_to(runtime) or not stdlib.is_relative_to(runtime):
        raise ValueError('Unsupported interpreter layout')
    identity = hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest()
    # -I excludes environment, user packages and the source checkout. Only the
    # copied tree is added explicitly. -S excludes installed site packages.
    driver = ('import sys,unittest; sys.path.insert(0,sys.argv[1]); '
              's=unittest.defaultTestLoader.discover(sys.argv[1],'
              'pattern=sys.argv[2],top_level_dir=sys.argv[1]); '
              'r=unittest.TextTestRunner().run(s); '
              'sys.exit(0 if r.testsRun>0 and r.wasSuccessful() else 1)')
    # Limit this initial experiment to one flat test module. Package discovery
    # and extra data/dependency support require a separately reviewed boundary.
    if '/' in suite:
        raise ValueError('Suite must be a top-level .py file')
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gents-python-check-') as temporary, \
            tempfile.TemporaryDirectory(prefix='gents-python-scratch-') as mutable:
        work = Path(temporary).resolve()
        scratch = Path(mutable).resolve() if scratch_bytes else None
        for name, body in contents.items():
            target = work / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
            target.chmod(0o400)
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            child = subprocess.Popen(
                ['/usr/bin/sandbox-exec', '-p', _profile(work, executable, runtime, scratch),
                 str(executable), '-I', '-S', '-B', '-c', driver, str(work), suite],
                cwd=work, env={'HOME': str(work), 'TMPDIR': str(scratch or work), 'LC_ALL': 'C'},
                stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                start_new_session=True, preexec_fn=lambda: _limits(
                    max(MAX_OUTPUT + 1, min(scratch_bytes, 1_048_576))))
            timed_out = False
            memory_limit = False
            scratch_limit = False
            scratch_used = scratch_files = 0
            try:
                deadline = time.monotonic() + timeout_seconds
                while child.poll() is None:
                    if time.monotonic() >= deadline:
                        timed_out = True
                        break
                    usage = subprocess.run(
                        ['/bin/ps', '-o', 'rss=', '-p', str(child.pid)],
                        capture_output=True, env={}, timeout=1, check=False)
                    rss = usage.stdout.strip()
                    if rss.isdigit() and int(rss) > 256 * 1024:
                        memory_limit = True
                        break
                    if not rss.isdigit() and child.poll() is None:
                        raise ValueError('Resident memory observation unavailable')
                    if scratch is not None:
                        scratch_used, scratch_files = _scratch_usage(scratch, scratch_bytes)
                        if scratch_used > scratch_bytes or scratch_files > 256:
                            scratch_limit = True
                            break
                    time.sleep(.02)
            finally:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
            stdout.seek(0)
            stderr.seek(0)
            out, err = stdout.read(MAX_OUTPUT + 1), stderr.read(MAX_OUTPUT + 1)
        if scratch is not None:
            scratch_used, scratch_files = _scratch_usage(scratch, scratch_bytes)
            scratch_limit |= scratch_used > scratch_bytes or scratch_files > 256
        unchanged = all((work / name).read_bytes() == body
                        for name, body in contents.items())
    limited = (timed_out or memory_limit or scratch_limit or max(len(out), len(err)) > MAX_OUTPUT
               or child.returncode < 0)
    return {'status': 'indeterminate' if limited or not unchanged else
            ('reported_pass' if child.returncode == 0 else 'reported_fail'),
            'source_identity': identity, 'inputs_sha256': hashes,
            'suite_sha256': suite_sha256, 'snapshot_unchanged': unchanged,
            'interpreter_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
            'python_version': sys.version.split()[0],
            'exit_code': child.returncode, 'timed_out': timed_out,
            'resident_memory_limit': memory_limit,
            'scratch_limit_bytes': scratch_bytes,
            'scratch_observed_bytes': scratch_used,
            'scratch_observed_entries': scratch_files,
            'scratch_limit_exceeded': scratch_limit,
            'output_truncated': max(len(out), len(err)) > MAX_OUTPUT,
            'stdout': out[:MAX_OUTPUT].decode('utf-8', errors='replace'),
            'stderr': err[:MAX_OUTPUT].decode('utf-8', errors='replace'),
            'elapsed_ms': round((time.monotonic() - started) * 1000),
            'model_calls': 0, 'provider_cost_usd': 0,
            'qualified': False, 'promoted': False, 'authority_granted': False,
            'scope': 'Host-only fresh snapshot experiment; no result reuse, grants, '
                     'company completion or factual correctness established. '
                     'Runtime library closure is not content-addressed.'}


def main():
    """Native host transport only: source bytes are data, never helper code."""
    payload = sys.stdin.buffer.read(1_600_001)
    if len(payload) > 1_600_000:
        raise ValueError('Native input budget exceeded')
    request = json.loads(payload)
    if (not isinstance(request, dict) or not {'files', 'suite', 'suite_sha256'} <= set(request)
            or set(request) - {'files', 'suite', 'suite_sha256', 'scratch_bytes'}):
        raise ValueError('Invalid native snapshot request')
    files = request['files']
    if not isinstance(files, dict) or not 1 <= len(files) <= MAX_FILES:
        raise ValueError('Invalid native snapshot files')
    # Native workspace confinement and UTF-8 admission precede this interface.
    # Revalidate names before materializing even these host-supplied copies.
    total = 0
    with tempfile.TemporaryDirectory(prefix='gents-python-inputs-') as temporary:
        root = Path(temporary)
        for name, text in files.items():
            relative = PurePosixPath(name)
            if (str(relative) != name or relative.is_absolute()
                    or any(p.startswith('.') for p in relative.parts)
                    or relative.suffix != '.py' or not isinstance(text, str)):
                raise ValueError('Invalid native Python input')
            body = text.encode('utf-8')
            total += len(body)
            if total > MAX_BYTES:
                raise ValueError('Native source byte budget exceeded')
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
        result = verify(root, list(files), request['suite'], request['suite_sha256'],
                        scratch_bytes=request.get('scratch_bytes', 0))
    print(json.dumps(result, ensure_ascii=True))


if __name__ == '__main__':
    main()
