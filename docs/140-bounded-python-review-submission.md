# Bounded Python repair-to-review submission

The company completed one local repair-to-review task in **108.38 seconds and
13 recorded model replies**, within an owner-frozen ceiling of 16 attempts.
Provider fees were **$0**. The worker used real native tools to inspect a failing
Python candidate, author its repair, obtain passing diagnostics and read it back.
An explicit owner-approved controller then proposed review-stage `finish`, which
passed ordinary admin and local QA review. The host separately inspected the
short function and checked the exact final source against six contract examples
and 300 deterministic generated string lists. Nothing was automatically promoted.

This supersedes neither the [earlier rejected loop](139-native-python-diagnostics-and-rejected-repair.md)
nor the unfinished observer task. It establishes one small candidate workflow,
not general autonomous coding, frontier equivalence or perpetual execution.

## Why context alone was insufficient

A preceding fresh trial used the same task, frozen four-case suite, grants and
local model configuration, with the new current-source diagnostic context. It
still retested and rewrote passing source instead of submitting it. Its ceiling
reserved exactly 16 attempts; 37 later admission denials caused no further model
dispatch. The owner cancelled it at a confirmed no-pending-effect boundary.
Its correct final source was not a completed submission.

That negative result motivated a control change rather than another prompt-only
repair. The successful fresh task explicitly approved a review-stage submission
program. Comparing these trials does not isolate one causal factor or establish
a general token/speed improvement.

## Owner-controlled attempt ceiling

Fresh intake may set `max_model_calls` to an integer from 1 through 1024. It is
included in the approved intake digest. The harness reserves each attempt in an
immediate SQLite transaction before model loading/context preparation. All roles
and model choices for that run share the allowance. Failed preparation,
interrupted calls, crashes and lost responses do not refund reservations.
Restarting cannot reset it, and duplicate reservation IDs cannot replay a call.

This is an attempt ceiling, not exact token accounting, provider execution proof
or authority. Pre-intake CEO conversations have no frozen job allowance. Existing
intakes without the optional field retain their original behavior; no active job
was silently assigned a new ceiling. The trial recorded 63,408 input tokens,
1,703 output tokens and 1,311 cached input tokens separately. Reasoning-token
accounting was unknown. Hardware, electricity, installed models and frontier host
development are excluded from $0 provider fees.

## Explicit review-stage control

The owner freezes this additional declaration before approving a fresh task:

```json
{
  "deliverable_type": "python_review_candidate",
  "max_model_calls": 16,
  "python_review_submission": {
    "version": 1,
    "actor": "Engineer Gent",
    "contract_id": "stable-unique"
  }
}
```

Its actor/ID must reference an ordinary frozen `python_check_contracts` entry.
The controller cannot be combined with the existing CEO/candidate workflow modes.
It creates no tool grants or code and does not change generic shell replay rules.

Before proposing submission, the host requires current-source `reported_pass`
diagnostics with their exact canonical receipt and no later uncertain attempt.
Every candidate source also needs an actual current-epoch native write and
complete readback. Paged reads must cover all current bytes without gaps.
Private custody records retain exact argument/result JSON, whose hashes must
match native audit receipts; payload edits, missing receipts, old source hashes,
other actors and old epochs cannot supply custody. Frozen tests stay protected.

The generated `finish` request says that independent correctness and promotion
remain unestablished. Existing admin, artifact and final QA gates still run. A
durable fingerprint prevents unchanged work from receiving repeated automatic
submissions; QA rejection is not permission to retry unchanged work. Raw Python
test success remains spoofable, so this controller is restricted to explicitly
approved review-stage candidates. It does not certify arbitrary Python behavior.

The worker still selects research/authoring actions and writes the code. This
controller removes one unnecessary free-form decision after mechanical
prerequisites have been satisfied. No live coaching or permission change was
used in the successful trial. Models were already installed; no downloads or
weight training occurred. The same local engine filled separate roles, so host
source review and extra checks remain material acceptance evidence.

## Verification and remaining scope

Native tests cover paged-read gaps, corrupted custody, current source binding,
privacy rejection, lost audit receipts and both MCP transports. Controller checks
reject missing writes/readbacks, failed diagnostics, uncertainty, stale policy
and revoked finish permission. Budget checks exercise concurrent reservations,
approval, wrong owners and persistence across reopened connections. Full offline
workspace tests, strict Clippy, a fresh build and broker checks passed.

The [public evidence](evidence/local-python-review-submission-2026-11-15.json)
retains measurements, source/binary identities, receipts, host acceptance and the
negative context-only trial. This small string-list function is a protocol case.
The exact [candidate source](evidence/python-review-candidate-2026-11-15.py) and
[host probes](evidence/python-review-host-checks-2026-11-15.py) are retained as
review artifacts. They are not installed runtime functions. The website build
verifies their hashes against the evidence before rendering the measured result.
The real observer implementation, richer I/O/dependency contracts, all three
tool-enabled orchestration modes, persistent improvement scheduling and qualified
media/weight learning remain separate work. Source and website deployment also
need their own publication evidence.
