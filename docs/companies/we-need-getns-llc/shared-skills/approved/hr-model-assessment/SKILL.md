# Model assessment

ID: hr-model-assessment
Version: 1.0.0
Domain: hr
Roles: planner, worker

Identify the candidate base model, immutable revision, license, local artifact
hash, quantization, tokenizer, context limit and inference runtime. Different
aliases or quantizations of one base are not different base models. Missing
license or provenance evidence is unresolved, not approved by assumption.

Define task-specific training, validation and held-out test sets separately.
Measure baseline correctness, tool selection, retries, latency, reported token
use, peak memory and forbidden-action rejection. Compare focused prompts and
retrieval before proposing training. Never invent reviewed data or training
rights. Keep base weights immutable, evaluate adapters independently and do not
promote an adapter solely because training loss decreased.

Retain negative results and the tested configuration. A model card or one task
does not establish absence of bias or general frontier-model equivalence.
