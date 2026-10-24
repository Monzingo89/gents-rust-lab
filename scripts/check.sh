#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 scripts/smoke.py
python3 scripts/reliability_smoke.py
python3 scripts/company_smoke.py
python3 scripts/autonomy_smoke.py
python3 scripts/task_evidence_smoke.py
python3 -m unittest discover -s scripts -p 'test_training.py'
python3 -m unittest discover -s scripts -p 'test_gents.py'
python3 -m unittest discover -s scripts -p 'test_company_tools.py'
python3 -m unittest discover -s scripts -p 'test_company_schedule.py'
python3 -m unittest discover -s scripts -p 'test_task_evidence.py'
node --test scripts/company-env.test.mjs
node --test scripts/gent-avatars.test.mjs
node --check web/app.js
npm run build
