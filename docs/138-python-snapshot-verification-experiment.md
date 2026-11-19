# Read-only Python retesting experiment

The observer task documented in [135](135-local-company-achievements.md) cannot
repeat an identical shell test after repairing source. This experiment provides
a narrower verification boundary without changing generic shell duplicate
suppression, any existing job, or approval and uncertain-effect handling.

`scripts/python_snapshot_check.py` began as a **host-authored experiment**, not a
company-authored candidate, accepted observer repair or completed
continuous-improvement controller. A subsequent native `python_check` adapter
now embeds the helper and exposes diagnostics through current owner contracts,
ordinary review, native privacy classification and hashed receipts. See the
[native guide](../mcp/README.md). No company runtime or local model was started
for the original experiment checks. Provider fees and model
calls were both zero; host development and electricity are separate.

The caller supplies at most 32 explicit Python inputs, one top-level unittest
suite and its previously frozen SHA-256. The adapter copies at most 256 KB into
a private temporary directory, rejecting symlinks, hard links, special files,
hidden/traversal paths and changing input reads. It runs the installed interpreter
directly with `-I -S -B`; only the copied input tree is added to Python's search
path. It neither installs dependencies nor executes a shell.

The macOS Seatbelt profile denies network access, reading the original source
directory, all workspace writes and child processes. Only the copied snapshot,
installed Python runtime and named system runtime locations are readable.
The host enforces CPU, output and wall-time limits, samples resident memory with
a 256 MiB rejection threshold, kills the owned process group and compares copied
source bytes afterward. Memory sampling is an observation limit, not a hard
allocation quota. Other platforms fail closed.

Fresh execution always occurs. There is no result cache, permission cache,
automatic retry, history deletion or source-identity migration. Input hashes,
interpreter binary hash and Python version are recorded separately. The runtime
library closure is not fully content-addressed; these identities must not be
used as a sound cross-run result-cache key. A repair in the original directory
cannot alter a running copied snapshot. Separate input reads are not claimed to
be a transactional snapshot of the original directory.

Statuses deliberately say `reported_pass`, `reported_fail` or `indeterminate`.
Python test code can replace unittest machinery or call `os._exit(0)`. A probe
demonstrates that a spoofed zero exit still has `qualified:false`,
`promoted:false` and `authority_granted:false`. This boundary contains execution;
it cannot establish factual correctness or independent acceptance of arbitrary
Python code. Raw stdout/stderr are untrusted and must pass the existing privacy
gate before any company journal, event or prompt receives them.

## Verification

Run from the repository root with the installed Python:

```sh
python3 -m unittest discover -s scripts -p test_python_snapshot_check.py -v
python3 -m unittest discover -s scripts -p test_company_tools.py
```

Six regression tests pass on this Mac, including a real bad-source/repair/retest
sequence against identical frozen suite bytes, actual network/host-I/O/source
mutation/subprocess denial probes, wall/output/memory limits, empty and broken
suites, explicit source selection and rejected paths/links/hash changes. The
existing broker has 31 passing checks, including protection of both experiment
files from agent source patching. Research-default and public-view suites also
passed (five and ten checks). These are host regression checks written during
implementation, not a preregistered independent model benchmark.

## Native continuation and remaining work

Keep the preserved observer task unqualified and unchanged. The diagnostic tool
now has owner-frozen explicit source/suite contracts and interpreter binary
identity, current actor/run/policy binding, native receipt persistence, privacy
classification and source-bound harness retry accounting. Actual stdio and
HTTP/SSE tests exercise bad-source/repair/retest, immutable test inputs, stale
source identities, actor/phase/grant denial, sensitive output rejection and an
injected audit failure. A lost success receipt remains an uncertain execution;
it is never converted into acceptance or silently replayed.

Independent correctness acceptance and a fresh explicitly scoped local-company
task remain separate requirements. Interpreter libraries and arbitrary external
dependencies are not fully pinned; no result cache or general dependency-complete
identity is claimed. Pending-intent restart/cancellation behavior still needs a
dedicated end-to-end company trial before claiming durable retest recovery.
The first actual company trial subsequently exposed a repair regression and was
stopped unqualified; its [retained negative result](139-native-python-diagnostics-and-rejected-repair.md)
also records the source-bound planning observations added afterward.
Do not infer shell idempotence from `unittest` command spelling or silently add
verification privileges to the old task. Rolling back this experiment requires
only reverting its source/check wiring; no existing journals need restoration.
