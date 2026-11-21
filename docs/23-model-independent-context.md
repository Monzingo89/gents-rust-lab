# Model-independent context and evidence

Completion/archive storage now has a separate local adapter, atomic per-agent checkpoints and transactional owner recall. Unqualified candidate submissions remain passive. See [the archive checkpoint contract](67-local-memory-archive-checkpoints.md) for the current lifecycle and evidence boundaries.

The harness retains its existing Rust provider adapters. All calls use a shared role/content message format and a versioned envelope carrying the frozen task, acceptance criteria, role capabilities, and a run-scoped verified handoff. This is a portable application contract, not a promise that different models have interchangeable tokenizers or inference caches.

`memory::SqliteSnapshots` stores versioned reference-only indexes in
private company SQLite. Raw context and dirty-generation updates commit
atomically; housekeeping flushes after 30 seconds of quiet with a 120-second
maximum dirty interval. These bounds depend on host scheduling. Source IDs,
hashes, tiers, recipient, contract and policy versions are bound together;
advisory model text is not copied into a second authority source. Live retrieval
still enforces current ACLs and returns fresh records during debounce. Permission
changes invalidate indexes and native cache identity immediately. Process-local
notifications cancel stale calls; final database checks and atomic staging also
reject stale output. Terminal checkpoints capture before cold archival. This
is not yet semantic summarization, distributed revocation, encryption, or
transitive information-flow tracking. See the campaign document for tested
boundaries and remaining enterprise gates.

`evidence::Ledger` separates executed facts from plans and advisory text. It records discovered dataset IDs and immutable revisions, inspected file inventories, downloaded file hashes, written-but-unverified artifacts, recent tool failures, and PDF inspection receipts. The checkpoint stores the full ledger privately under the run; numbered tool journals retain the full original results. A bounded projection goes to each model. Startup rebuilds older checkpoints from their journals. Switching models discards inference cache without discarding task evidence.

`hf_dataset_files` searches and pages an already inspected source inventory from the original local tool journal. It returns exact observed paths, immutable revision, original inspection step, total matches, and a continuation offset. It makes no network request. Large inventories therefore remain available without placing thousands of filenames in every model's prompt.

Source validation requires an observed repository/revision and inspected exact file path before a dataset download. A valid missing inspection becomes a proposed prerequisite and still passes independent admin review. No tool result expands the frozen task or grants authority.

Recovery queues missing source inspections from verified IDs before another speculative worker turn. Failed read-only observations are deduplicated by their capability implementation revision, allowing bounded retry after a backend/security repair. Successful reads and all writes retain their deduplication across revisions. Answering a stall question resets its no-progress counter. A shell process with a nonzero exit code is a failed action, even when the broker itself ran successfully.

`context::compact` removes optional retrieved history before dispatch and preserves the frozen task, acceptance criteria, exact requested action, and verified handoff. The owned llama.cpp runtime applies the selected model's native chat template and tokenizer before inference and reserves output capacity. If required context cannot fit, it reports the problem instead of truncating the mission. MLX also uses the installed offline tokenizer helper for scoped tasks or an explicit context limit, reserving generation capacity before inference. Remote adapters retain envelope and byte compaction; this is not a claim of universal tokenizer compatibility.

The constrained local decoder derives its actions from the same approved tool catalog used by the broker and role capability repository. Adding a tool no longer requires editing a second hardcoded decoder list. Planner and worker visibility remains role-scoped; execution still requires authorization of the exact next action.

## Structured tools and recovery boundaries

`contracts::ToolContracts` supplies core input schemas from `config/company-tool-inputs.json`. The native decoder constrains the selected action and its structured arguments, including required source IDs/revisions, file operations and bounded page ranges. The same schemas are visible to models and validated before admin review. Old `arguments_json` checkpoints and capabilities without a registered input schema remain compatible and retain broker validation. The native decoder also supports sequential plans of up to four calls, each independently reviewed. This uses llama.cpp's documented [JSON Schema grammar subset](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/grammars/README.md); it is not a claim of complete JSON Schema implementation.

Planners can inspect every capability as `plan_only`; workers request execution. Repeated per-tool role and authority boilerplate was removed from model inputs, while full tool descriptions and input schemas remain. The frozen task, exact request and verified handoff survive compaction. Duplicate progress indexes and old manuals are removed before recent observations. The latest answer to a question supersedes earlier copies of that same answer thread.

Ledger version two retains both successful inventory selections and empty-query evidence. Negative queries cannot evict all useful reference selections. Original full results remain in private numbered journals, and old checkpoints rebuild the projection once. Workspace context lists actual files and distinguishes downloaded references from unverified workspace output. Paths passed to file/PDF/shell capabilities are relative to this workspace.

Native compaction recounts the actual tokenizer after removing optional content. It does not reject a safely reduced prompt solely because a coarse byte-reduction target was missed. If required content still exceeds native capacity, the error reports its token count and available prompt budget. Company retrieval uses the frozen task instead of recursively querying prior retrieved text. Unmatched onboarding documents are excluded from the recency fallback for a specific task; explicit empty-query company browsing remains available.

Recovery blocks exact repeated requests rather than disabling an entire parameterized tool family. A repeated file listing therefore cannot hide a subsequent write or a different read. Only exhausted closed parameterless tools receive a family cooldown. Empty inventory queries, journal reads and workspace listings do not reset artifact-progress recovery or permanently replace the coding worker. Temporary recovery models return control to the configured worker with shared verified facts; deliberate `model_select` overrides are retained.

Program shutdown records an interrupted company job for checkpoint recovery. Explicit operator cancellation remains cancelled until authorized resume. `POST /api/runs/{id}/resume` validates ownership, the original approval and frozen-intake hash, preserves the run ID and journal, rejects completed jobs and duplicate execution, and retains any uncertain pending effect for resolution. It does not replay that effect. HTTP regression coverage exercises supervised restart, ordinary process termination, explicit cancellation and idempotent resume.

## Document capability

Install local document dependencies with `uv pip install --python .venv-inference/bin/python -r config/document-requirements.txt`. Poppler's `pdftoppm` must be installed on the host. The isolated `shell` tool can use the document Python environment and its installed base interpreter, but cannot read arbitrary home files, write outside the run workspace, or access the network.

Local agents author content themselves using `files` and `shell`. `pdf_inspect` opens an existing workspace PDF with pypdf, extracts bounded page text, renders up to eight pages per call, and returns its SHA-256 and page count. These receipts prove parsing/rendering, not buildability or visual correctness. The completion gate requires receipts covering every page of the requested output at its current hash and excludes downloaded source examples. A change to the PDF invalidates prior coverage. The configured QA worker, rather than the authorization model, performs the final independent acceptance review.

For tasks explicitly requesting Hugging Face PDF reference examples, recovery can propose downloading one exact observed small reference and opening its pages before returning to output authoring. Both operations still pass the admin gate. A reference is never counted as a new deliverable. Filename search accepts literal and punctuation-normalized fragments while returning the exact original paths.

The first downloaded reference exposed a separate viewer bug: it permits blank-password opening while retaining PDF copy restrictions. Inspection now distinguishes that case from a required opening password, preserves the original encrypted file, honors the extraction permission, and renders viewable pages. It never guesses a nonempty password. This uses pypdf's documented [opening-password and permission behavior](https://pypdf.readthedocs.io/en/latest/modules/PdfReader.html). Inspection failures return controlled error codes. The helper runs with the shared restricted workspace environment and no inherited integration credentials. Tests cover copy restrictions, rejected passwords, permission integrity, stale page-range renders, and prevention of writes through workspace symlinks to host files.

Rendered image paths alone are not visual-model inspection. References with restricted or image-only text require a local visual review capability before visual findings can be claimed. The resumed tower task still requires its authored outputs and independent task-specific QA; neither a downloaded reference nor these renderer checks satisfy that goal.

`image_review` now invokes the installed `llama-mtmd-cli` and local `models/qwen3-vl-8b/` base/projector files through a small vision-backend interface. It shares the GPU lease, retains the workspace sandbox and credential isolation, and records correlated model-call spans. CLI output stays in memory until data assessment; only generated stdout becomes an observation, never stderr diagnostics. Do not use `--log-disable`: the installed CLI also suppresses generated stdout with that flag. Exact CLI token counts are unavailable and remain null. Hash-bound advisory observations survive model handoffs without becoming verified task facts. Reference recovery can propose a real visual call for rendered pages with no extractable text; the normal admin gate still applies. Final QA must also inspect actual finished artifact content and verify rubric arithmetic; metadata presence and a claimed score alone are insufficient. MLX tokenizer budgeting and privileged self-modification boundaries still require their own verification before broader deployment claims.

The document completion gate now checks explicit rubric arithmetic and binds the score to current PDF/inventory/evidence hashes. `document_review::ArtifactEvidenceSource` supplies complete text files in bounded UTF-8 chunks and PDF page content from the original inspection journals. Every output page needs hash-matched visual findings; missing or changed images fail the content check. An independent QA model reads each packet before final cross-document review. Packet reviews are cached by content and artifact-set hashes, so unchanged packets do not incur repeated model calls during repair. Tests cover stale artifacts/images, incorrect score arithmetic and complete multibyte text chunking. This is review infrastructure, not evidence that any particular task has passed QA.

An actual brick-building deliverable still needs real-part provenance, consistent inventory and construction steps, stability and followability checks, explicit rubric scoring, and visual review. Unit tests and a model's claimed score do not establish these properties.

## Observed source-metadata filter failure

The live retry reached dataset inspection, but the original payment-card detector extracted 13-digit substrings from alphanumeric PDF filenames and discarded both inventories. The detector now checks complete numeric token boundaries and retains standalone card-number checks, including spaced/hyphenated values and numeric filenames. Regression tests cover both cases. This heuristic remains a scope-avoidance control; it is not evidence of PCI DSS, HIPAA, or comprehensive sensitive-data detection.

Dataset downloads temporarily enable network access only in their authenticated download subprocess. Local inference and isolated authoring shells retain their offline settings. Exact positional filenames avoid interpreting an observed filename as an include glob.
