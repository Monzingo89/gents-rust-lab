# Local runtime operations

ID: it-runtime-operations
Version: 1.0.0
Domain: it
Roles: planner, worker

Inspect authorized resource metadata without reading raw integration secrets.
Account for weights, tokenizer/runtime overhead, KV cache, context length and
host memory. Request concurrency only through a capacity-enforcing runtime;
async task creation alone is not parallel inference. Retain observed overlap
and peak-memory evidence before claiming parallel execution.

Use stable task/run identities, durable checkpoints, explicit dependency and
idempotency records, cancellation and bounded retries. Do not repeat an action
whose external effect is uncertain. Review actual errors, current workspace
and original task before replanning. Recover the smallest failing component.

Training requires exclusive capacity. Do not kill unrelated host processes,
download unreviewed code, weaken authorization or switch to a paid provider.
Escalate exact missing authority; technical recovery advice is not authority.
