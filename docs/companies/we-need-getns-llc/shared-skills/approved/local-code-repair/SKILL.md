# Local Code Repair

Use this procedure only when evidence shows the harness or a local tool is defective, not merely because a source is unavailable or a task is difficult.

1. Reproduce the failure and retain its exact observable evidence.
2. Use `source_read` on the smallest relevant allowlisted file and retain its SHA-256.
3. Design a minimal change that preserves tenant isolation, secret handling, the Admin Judge, and sensitive-data gates.
4. Use `source_patch` with the exact prior SHA-256 and complete replacement content. The tool backs up the file, runs fixed checks, and rolls back a failing change.
5. Use `harness_restart` only after a successful patch when the running binary must reload. The supervised office retains checkpoints and retries from verified state.
6. Verify the original failure after restart. Never describe a proposed patch or a passing unrelated test as a completed repair.

Planners may inspect and propose repairs. Workers may request mutation tools. The skill does not authorize scope changes, secrets, new providers, security-policy weakening, destructive operations, or edits outside the allowlisted repository paths.
