# Local task evidence and the contractor audit

This is a bounded, deterministic tool for the fictional `we-need-gents-lc` mobilization task, followed by the existing Gent review chain. It is not a general autonomous tool runner or compliance certification. No API keys, external inference, automatic model downloads, or owner answer key are used.

## Prepare and review

Restart the server to load the new evidence endpoints: stop the existing harness in its terminal with Ctrl+C, then run `sh scripts/run-company.sh`. Do not stop unrelated services or start a second copy on port 8788.

In another terminal:

```sh
python3 scripts/gents.py --company we-need-gents-lc prepare-task
```

This copies only allowlisted files from `company/` and `task/` into a new private company evidence package. Original documents never move. The operator tool uses installed FFmpeg, cached local Whisper `tiny.pt` on CPU, and Apple Vision OCR via Swift. Both audio streams are transcribed. Video frames are extracted at integer-second timestamps and retain OCR observations. Inputs are bounded to 100 MB per file and five minutes per media file. This fixture's static ledger screens are supported; one-second sampling is not sufficient proof of arbitrary future video completeness.

The command prints a package ID and directory under `data/companies/we-need-gents-lc/evidence/`. Inspect the original media, timestamped frames, transcript JSONs and `media-review.json`. Transcription can misrender dates, so it is never silently treated as verified policy.

```sh
python3 scripts/gents.py --company we-need-gents-lc review-task PACKAGE_ID
```

The review command prints the machine transcripts and proposed corrections. Attest only after checking both original audio streams and the referenced frames. Enter your reviewer label and the effective hire date, training deadline and manager-approval rule actually stated in the authorized notice. Review is an operator evidence attestation, not a legal signature or an HR approval. Unreviewed media cannot enter a frozen audit intake.

## New scope, approval and execution

The review command prints the next intake command:

```sh
python3 scripts/gents.py --company we-need-gents-lc intake --task-file docs/companies/we-need-getns-llc/CLI-AUDIT-TASK.txt --evidence-package PACKAGE_ID
```

`CLI-AUDIT-TASK.txt` is a narrower executable audit scope. The original `TASK-TO-PASTE.txt` additionally asks for a demonstrated Gent sleep/wake cycle and broader skill/lifecycle work; this audit tool does not establish those behaviors. Review the new SOW before authorizing it. Do not silently overwrite or reapprove the previous incomplete run.

Choose `text`. Suggested acceptance criteria, entered separately:

1. Cover all 12 employee IDs exactly once and reconcile request/register joins.
2. Cite source facts, policy sections, reviewed audio intervals and correction frames.
3. Apply policy dates, four department gates and exact release-ticket formatting.
4. Produce all five artifacts and pass deterministic checks; report unknowns and limitations.

Use no deadline unless you need one. Scope excludes paid services, deployment and real signatures. Use simulation signatory labels if appropriate. Open constraints should say evidence is attached and reviewed, not repeat the old missing-files warning. The evidence manifest hash is part of the frozen intake digest.

Use the exact approval command the CLI prints, then:

```sh
python3 scripts/gents.py --company we-need-gents-lc start RUN_ID
python3 scripts/gents.py --company we-need-gents-lc tail RUN_ID
```

Start first checks operator approval and server capability. It validates the frozen evidence, executes the deterministic audit and independent control-flow verifier, and attaches the task facts/results to the selected run before starting model reviews. It generates `decisions.csv`, `findings.json`, `evidence.json`, `report.html` and `run-summary.json` under `data/companies/we-need-gents-lc/runs/RUN_ID/audit/`. The office log includes an actual `Audit Tool` result with the artifact directory. Models do not get a shell or arbitrary filesystem access. No real accounts or payments change.

The rule compiler supports the company policy clauses shipped with this fixture. Changed or unsupported policy text must fail for review rather than silently adapting legal/business rules. The separate verifier recomputes outcomes and checks source facts, dates, gates, schema coverage, discrepancy values, citations and hashes without using owner ground truth. This is not an independent accuracy score or proof of every future policy interpretation.

## Reverification, cache and isolation

```sh
python3 scripts/gents.py --company we-need-gents-lc verify-audit RUN_ID
python3 scripts/gents.py --company we-need-gents-lc prepare-task --package PACKAGE_ID
```

Package reuse validates retained file hashes and review hashes before reporting a real cache hit. Repeating execution for the same approved run validates existing outputs rather than overwriting them. Changed frozen evidence requires a new reviewed intake.

Task evidence is a run-scoped `EVIDENCE` tier, not company-wide onboarding PSTC. Other runs and tenants cannot retrieve it. Completion/archive/recall preserve this run boundary using `EVIDENCE_ALTC` and `EVIDENCE_PLTC`. Explicit model handoffs still follow the existing company-scoped lifecycle. Raw evidence is never globally promoted. Server admission and final acceptance check the frozen package and actual artifact hashes; a model verdict cannot override failed objective checks.

Artifacts distinguish deterministic tool work (zero LLM calls/tokens) from model reviews, whose actual usage remains in harness events. Validated cache reuse is not a simulated Gent sleep/wake cycle. No production authorization, legal certification, general software execution, or autonomous tool admission is implied.

## Verification

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo test --workspace --locked
python3 scripts/task_evidence_smoke.py
```

The HTTP smoke uses a disposable root and synthetic media interpretations. It does not approve or alter a real company's SOW or claim that real media review has happened.

Implementation references: [Whisper](https://github.com/openai/whisper), [Apple Vision text recognition](https://developer.apple.com/documentation/vision/vnrecognizetextrequest).
