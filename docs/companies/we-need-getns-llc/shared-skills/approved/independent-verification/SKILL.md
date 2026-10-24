# Independent verification
ID: independent-verification
Version: 1.0.0
Status: APPROVED-DEMO
Scope: reusable procedure, not company policy
Inputs: assigned task, source catalog, permitted tools
Outputs: structured results, evidence references, unresolved issues
Procedure: Read acceptance criteria and actual output artifacts in a fresh context. Validate schemas, coverage, calculations, evidence content and unresolved dependencies. Return pass/revise/uncertain/capability_gap. Deterministic failures cannot be overridden by a model verdict.
Validation: require source locators, actual execution evidence where applicable, and explicit failure states.
Permissions: inherits the harness tool allowlist; cannot expand it.
