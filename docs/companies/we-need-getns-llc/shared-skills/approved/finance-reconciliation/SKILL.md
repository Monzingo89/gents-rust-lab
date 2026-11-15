# Synthetic finance reconciliation

ID: finance-reconciliation
Version: 1.1.0
Domain: finance
Roles: planner, worker

Use only the assigned synthetic records. Define the input/output schema and
currency semantics before implementation. Use integer minor units or exact
decimal arithmetic, stable record IDs and explicit currency fields. Treat
missing values, duplicates, mismatched currencies and inconsistent totals as
exceptions with source references, not facts to invent or silently repair.

Use the approved integer_arithmetic capability for exact sums and differences
when assigned; do not rely on a model's unverified mental arithmetic. Preserve
the tool receipt and apply financial status policy independently to the original
currencies, identifiers and record presence. A zero difference does not erase a
currency or duplicate exception. If batching causes missed prerequisites, use
smaller case-scoped worker turns without dropping any original cases or tests.

Hand off a compact versioned contract and artifact references. Verify actual
output against deterministic held-out cases, including boundary and negative
cases. The independent reviewer must execute tests rather than accept the
builder's claimed results. Retain failure receipts and bounded repair history.

This procedure does not authorize transfers, credit decisions, account access,
live banking data, financial advice or a compliance conclusion.
