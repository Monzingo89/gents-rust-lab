# Fast answers and bounded public requests

Common setup answers are shared between the public website and the local CEO panel through `web/assets/basic-answers.js`. Choosing a question renders the maintained project answer immediately: no model, planner, tools, paid API or network request. These are known setup answers, not arbitrary AI answers. Unknown/ambiguous text is never guessed. The normal local CEO workflow still handles task planning with its existing budgets and permissions.

The hosted site contains no public model/tool execution endpoint. Static pages, calculators, hardware-report import and task-brief preparation stay in the browser. The only dynamic endpoint is the separately authorized support form.

## Support safeguards

`site/server/request-guard.mjs` separates admission, bounded body reading and delivery deduplication from provider policy and HTTP handling. All limits below are host-owned, not model-controlled:

| Limit | Behavior |
| --- | --- |
| Trusted metadata | Reject unsupported methods, exact-origin mismatches, invalid platform IP headers, unsupported content types/encodings and oversized declared bodies before reading the body |
| Body | Maximum 8 KiB, including requests without Content-Length; 2-second body-read deadline |
| Warm-process admission | 8 accepted requests/client/minute, 60 total/minute, 2 concurrent requests; reject excess with 429 and Retry-After |
| Client tracking | At most 512 hashed identities/window; refuse new identities when full; no eviction to reset active quotas |
| Delivery replay | Reserve before provider invocation; one attempt/key/24 hours in this process, including uncertain delivery; at most 256 retained attempts, fail closed when full |
| Provider | Existing 8-second request timeout, 10-second platform function limit, fixed endpoint/recipient and provider idempotency; no automatic retries |

Counters and the replay ledger live in each warm function instance. They reset on cold starts and are independent across instances/regions. They are **not a distributed quota or spend guarantee**. The existing provider idempotency key continues to protect matching connection/day deliveries across instances. Shared connections share limits.

Vercel's CLI reported platform system protections active. Custom WAF rules, attack mode and managed bot protection were not enabled. No system protection was paused and no bypass was created. A missing custom firewall configuration is not evidence that system DDoS protection is off.

The [Vercel firewall documentation](https://vercel.com/docs/vercel-firewall) describes its platform DDoS mitigation. Its [pricing documentation](https://vercel.com/docs/vercel-firewall/vercel-waf/usage-and-pricing) distinguishes free protections from billed rate limiting; no billed rate-limit service or plan upgrade was activated. Distributed abuse and normal hosting usage can still exhaust platform quotas. No availability or zero-hosting-cost guarantee is made. Stronger distributed admission is still release work.

## Verification

```sh
node --test scripts/request-guard.test.mjs scripts/basic-answers.test.mjs scripts/support.test.mjs scripts/introduction.test.mjs scripts/computer-readiness.test.mjs
node scripts/build-introduction.mjs --vercel-preview
python3 scripts/verify_introduction.py
node scripts/build-vercel-introduction.mjs
node --check site/deploy/api/support.js
```

The guard tests exercise burst/global/concurrency exhaustion, bounded client tracking, expiry, early rejection before body access, stalled/oversized streams, slot release, delivery uncertainty and replay suppression. The browser smoke verifies immediate setup-answer rendering with no added network/model calls alongside the existing desktop/mobile onboarding flow. Website checks do not send email, execute tasks or load new models. This is bounded verification, not a volumetric attack test.
