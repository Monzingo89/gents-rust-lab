#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
command -v uv >/dev/null || { echo 'Install uv first: https://docs.astral.sh/uv/'; exit 1; }
command -v llama-server >/dev/null || { echo 'Install llama.cpp: brew install llama.cpp'; exit 1; }
if [ ! -x .venv-inference/bin/python ]; then uv venv .venv-inference --python 3.12; fi
uv pip install --python .venv-inference/bin/python -r config/inference-requirements.txt
python3 scripts/gents.py doctor
