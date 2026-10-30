#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Keep the existing unrelated service on 8787 undisturbed.
export AUTH_MODE=local BIND=127.0.0.1:8788 CONFIG=config/company.local.json
export GENTS_LOCAL_ONLY=1 HF_HUB_OFFLINE=1 HF_HUB_DISABLE_TELEMETRY=1 TRANSFORMERS_OFFLINE=1
export RUN_STORE=data/company-runs.sqlite3
cargo build -p gents-harness --locked
exec node scripts/run-harness.mjs
