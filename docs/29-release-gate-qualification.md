# Local release gate: architectural repair and counterfactual results

## Scope

This increment tests the local synthetic banking pilot, not real banking,
healthcare or regulatory certification. An earlier three-case risk diagnostic
was too narrow to establish reliable gate behavior. No training labels or
expected verdicts are included in the model requests below.

## Architecture

`release_gate.rs` separates input contracts, source-derived decoder bounds,
independent answer checking and banking completion admission. Its interface is
provider-independent and performs no I/O. The native model adapter binds only
the supplied correlation identity, all possible control names and sufficient
response capacity. It does not select failed controls or constrain the verdict
to an expected answer.

The former four-entry blocker limit could not express seven simultaneous
failures plus a production refusal. Capacity is now the number of supplied
controls plus one, bounded at 32 controls. Empty, non-boolean, oversized,
reserved-name and malformed controls fail closed. Banking completion requires
synthetic scope and all seven named measured controls; an empty/subset control
object or production request cannot pass through vacuous truth. The native
protocol emits blockers before deciding release, but the independent checker
still rejects an incorrect result rather than repairing it.

Every one of the 128 subsets of seven controls was tested under synthetic and
production scope: 256 deterministic scenarios. Incorrect verdicts, omitted or
duplicate blockers and incomplete admission controls are rejected. These
component checks do not imply the model itself passes those scenarios.

## Actual local model results

Both 33-call diagnostics used the pinned Gemma risk model, the same transport
ID across counterfactuals, every single-false/single-true control, all-false and
all-true sets, synthetic/production scopes and one unknown scope. Native
execution receipts are present for all calls; their reported provider charges
sum to USD 0, excluding hardware, electricity and coaching.

| Trial | Exact passes | Result retained |
| --- | --- | --- |
| Expanded baseline with corrected capacity | 19/33 | `data/model-registry/risk-identity-probe-82c4067e-fe6f-48d0-9d29-3aace2d77b68.json` |
| Blockers-first protocol and source vocabulary | 25/33 | `data/model-registry/risk-identity-probe-8f69f5f1-8b17-41de-be42-1147c1ffdb7a.json` |

Both are FAIL, not qualification passes. The baseline missed a failed ceiling
control. The revised trial still missed a failed owner-isolation control,
misstated blockers in two other cases and incorrectly ordered five lists.
The independent gate rejects these outputs. Better exploratory performance is
not a held-out qualification claim or a basis for removing deterministic
checks. No weights were trained and no paid inference was used.

## Readiness and repair-boundary hardening

Runtime admission now revalidates the current canonical role-contract hash,
model identity/weight hash, original case hashes, actual replies, local execution
receipts and zero provider charges. The offline indexer applies the same
canonical role hash. Old receipts without this binding remain historical
evidence but no longer count as current readiness. A cached ready flag cannot
override missing or stale proof.

The first integrated run after enabling this guard, `hard-515cea031d3b`, stopped
at intake admission because its legacy receipt lacked the role-contract hash.
This is a retained FAIL, before financial inference, not a banking result.
It demonstrated the intended fail-closed behavior and triggered fresh native
readiness checks rather than grandfathering old receipts.

The fresh campaign
`data/model-registry/role-campaigns/1112d974c47e494cbb02400880064edc`
passed 20 local calls across all ten models in 184.91 summed per-model seconds.
The refreshed index reports ten ready and zero broadly enterprise-qualified
models. The subsequent full run `hard-60d2f98e19f9` passed ten cases and ten
protected memo actors with 76 fresh calls, no failed/cancelled calls and zero
provider charges in 354.41 seconds. Its artifact SHA-256 is
`097aab71b1ab48ac78f074170b86f610e7bd8bbb34151221022437e48584c62d`.
Its [short methods/results paper](../data/capability-campaigns/hard-60d2f98e19f9/hard-stage-paper.md)
separates this positive-path regression from the failed risk qualification.

The new gate is excluded from autonomous source repair. Related negative tests
also found and closed path-spelling bypasses in the existing protection list:
double separators, dot aliases, case aliases on case-insensitive filesystems,
and hard-linked source files. Ordinary allowlisted repair remains hash-guarded
and backed up. This is an application boundary, not protection from a hostile
same-user host process or proof against every filesystem race.

## Remaining acceptance work

The risk model is not broadly enterprise-qualified. Further decomposition or
model/workload evaluation must preserve these failures, use held-out cases and
retain deterministic authority. The passing ten-model banking regression and
current-contract readiness refresh do not override this failed counterfactual
diagnostic. Ten protected generated memos still do not prove that every
financial stage consumed them. Subsequent `hard-415cb1d2f001` proves actual
source readback by all ten models during workflow assurance, not per-financial-
decision summary use or semantic entailment. See
[the integrated consumption results](30-governed-memory-consumption.md).
Real regulated data and production effects remain
denied. The overall architecture goal remains active.

## Subsequent architectural decomposition

The opt-in compiled-policy path now passes a focused two-call native probe
and integrated run `hard-055a7b4542dd`. The risk model implements the English
owner contract as a bounded declarative program, which an interpreter executes
without repairing wrong rules. Independent gates remain authoritative.
Normal/suspended programs passed 384 checks each; the fresh integrated program
passed 384 checks and actual-input comparison. The run passed ten cases, ten
models and ten memory consumers with 96 new calls and no call failures.
This does not change the retained 19/33 or 25/33 raw judgment failures, qualify
the model generally, or permit production use. [Method and scope](31-local-policy-compilation.md).
