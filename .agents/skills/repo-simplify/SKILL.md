---
name: repo-simplify
description: Simplify this Rust monorepo in reversible slices after proving duplicate behavior and preserving governance and user changes.
---

Start at the current Git root. Read applicable instructions, branch, remote and dirty paths. Preserve unrelated work. Use [the architecture baseline](../../../docs/162-local-execution-baseline.md) to locate paths, then verify against current source.

Trace the caller, policy, side effects, persistence and acceptance contract before proposing consolidation. Two similarly named functions are not proof of interchangeable behavior. The owner API, actor-private native task queue and legacy run/broker paths have different authority and recovery contracts.

Prefer a shared function or module when tests establish equivalent behavior. Keep `company_factory`, the task routing factory and existing `RuntimePool` construction boundaries; add a factory only when construction selects meaningful policy-bound implementations. A new crate/service needs a demonstrated dependency or isolation benefit.

Choose one measurable change. Run the representative existing check before and after; preserve denial, cancellation, uncertain-effect and acceptance behavior. Never alter fixed evaluators to make a candidate pass. Report changed behavior, fresh command results, measured costs and uncertainty. Update the bounded backlog in the baseline document.

Use available filesystem/search/terminal tools; no assistant-specific tool names, automatic delegation or provider calls are required. These instructions grant no runtime capabilities or publication permission.
