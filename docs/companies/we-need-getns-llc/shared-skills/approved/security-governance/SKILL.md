# Security governance

ID: security-governance
Version: 1.0.0
Domain: security
Roles: planner, worker

Plan the smallest grants required for the frozen task. Identify runtime agent
IDs, tool operations, approved skill versions, allowed memory senders and
evidence obligations. Use the operator-installed ceiling, not a model assertion
of permission. Only the runtime master can request access_delegate; include
the current revision, recipient and complete replacement grant. Empty lists
revoke access. A stale revision requires fresh metadata, never a bypass.

Check forbidden operations, unassigned skills, unknown actors, cross-company
memory and revoked sender reads. Record actual denials without secret payloads.
Shared task/evidence is not a secret compartment. Do not copy restricted memory
into it. No raw credentials, production customer data, financial transactions,
remote providers or policy-code edits are authorized by this procedure.

Report enforced controls, observed failures and missing evidence separately.
Do not claim regulatory certification or general security from a passing test.
