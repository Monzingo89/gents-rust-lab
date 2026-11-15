#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 -m unittest discover -s scripts -p 'test_company_research.py'
python3 -m unittest discover -s scripts -p 'test_local_sqlite.py'
python3 -m unittest discover -s scripts -p 'test_completed_memory_proof.py'
python3 scripts/audio_inspect_interop.py
python3 scripts/smoke.py
python3 scripts/reliability_smoke.py
python3 scripts/company_smoke.py
python3 scripts/planning_model_policy_smoke.py
python3 scripts/owned_discovery_smoke.py
python3 scripts/company_factory_smoke.py
python3 scripts/coordination_smoke.py
python3 scripts/autonomy_smoke.py
python3 scripts/intent_retention_smoke.py
python3 scripts/qa_recovery_smoke.py
python3 scripts/frozen_artifact_gate_smoke.py
python3 scripts/frozen_artifact_gate_smoke.py --approved-workflow
python3 scripts/review_protocol_smoke.py
python3 scripts/rust_proposal_feedback_smoke.py
python3 scripts/rust_submission_smoke.py
python3 scripts/rust_submission_smoke.py --sse
python3 scripts/rust_submission_smoke.py --owned
python3 scripts/rust_submission_smoke.py --owned --sse
python3 scripts/task_evidence_smoke.py
python3 -m unittest discover -s scripts -p 'test_training.py'
python3 -m unittest discover -s scripts -p 'test_gents.py'
python3 -m unittest discover -s scripts -p 'test_company_tools.py'
python3 -m unittest discover -s scripts -p 'test_public_network.py'
python3 -m unittest discover -s scripts -p 'test_pdf_tools.py'
python3 -m unittest discover -s scripts -p 'test_vision_tools.py'
python3 -m unittest discover -s scripts -p 'test_company_schedule.py'
python3 -m unittest discover -s scripts -p 'test_task_evidence.py'
python3 -m unittest discover -s scripts -p 'test_mcp_curriculum.py'
node --test scripts/company-env.test.mjs scripts/company-env-audit.test.mjs
node --test scripts/architecture-review.test.mjs scripts/support.test.mjs scripts/computer-readiness.test.mjs scripts/request-guard.test.mjs scripts/basic-answers.test.mjs
node --test scripts/public-network.test.mjs
node --test scripts/gent-avatars.test.mjs
node --test scripts/office-activity.test.mjs
node --test scripts/room-activity.test.mjs
node --test scripts/ceo-execution.test.mjs
node --test scripts/ceo-source-files.test.mjs
node --test scripts/owned-invocations.test.mjs
node --test scripts/owned-selection.test.mjs
node --test scripts/mcp-team-verification.test.mjs
node --test scripts/mcp-curriculum-cli.test.mjs
node --test scripts/mcp-curriculum-resume.test.mjs
node --test scripts/mcp-workflow-review.test.mjs
node --test scripts/mcp-fixture-files.test.mjs
node --test scripts/mcp-coding-review.test.mjs
node --test scripts/mcp-source-lineage.test.mjs
node --test scripts/mcp-evaluation-host.test.mjs
node --test scripts/mcp-component-store.test.mjs
node --test scripts/mcp-qualified-component.test.mjs
node --test scripts/mcp-component-probe.test.mjs
node --test scripts/mcp-owned-fixture.test.mjs
node --test scripts/mcp-owned-prepared.test.mjs
node --test scripts/mcp-owned-edges.test.mjs
node --test scripts/mcp-owned-branched-allocation.test.mjs
node --test scripts/mcp-index-weights-fixture.test.mjs
node --test scripts/mcp-owned-multi-library.test.mjs
node --test scripts/mcp-owned-selection.test.mjs
node --test scripts/mcp-owned-catalog.test.mjs
python3 -m unittest discover -s scripts -p 'test_model_security_scan.py'
node --test scripts/model-security-service.test.mjs
node --test scripts/mcp-evaluation-profile.test.mjs
node --test scripts/mcp-owner-authorization.test.mjs
node --test scripts/mcp-runtime-fingerprint.test.mjs
node --test scripts/mcp-runtime-bundle.test.mjs
node --test scripts/mcp-weighted-slot-inputs.test.mjs
node --test scripts/mcp-settlement-inputs.test.mjs
node --test scripts/mcp-rust-acceptance.test.mjs
node --test scripts/mcp-planning-observer.test.mjs
node --test scripts/mcp-planning-continuation.test.mjs
node --test scripts/mcp-planning-policy-verifier.test.mjs
node --test scripts/mcp-observation.test.mjs
node --check scripts/mcp-curriculum.mjs
node --check scripts/mcp-workflow-review.mjs
node --check scripts/mcp-workflow-verifier.mjs
node --check scripts/mcp-coding-check.mjs
node --check scripts/mcp-coding-verifier.mjs
node --check scripts/mcp-transcript.mjs
node --check scripts/fixtures/mcp-rust-ranges.mjs
node --check scripts/fixtures/mcp-rust-cent-allocation.mjs
node --check scripts/fixtures/mcp-rust-dag-layers.mjs
node --check scripts/fixtures/mcp-rust-weighted-slots.mjs
node --check scripts/mcp-development-curriculum.mjs
node --check scripts/mcp-curriculum-bridge.mjs
node --check scripts/fixtures/mcp-workflow-catalog.mjs
node --check scripts/mcp-workflow-check.mjs
node --check scripts/fixtures/mcp-reconciliation.mjs
node --check web/app.js
npm run build
npm run test:site

node --check scripts/fixtures/mcp-depot-reconciliation.mjs
