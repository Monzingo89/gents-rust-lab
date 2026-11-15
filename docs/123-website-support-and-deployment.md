# Website and support operations

The evaluation website is **https://metivolve.com**; `www.metivolve.com` redirects to it. The original Vercel alias remains available. See [the new-user journey](125-metivolve-onboarding.md). It is published separately from the local company. The original evaluation deployment was unindexed; the public source release enables production indexing (see [release operations](132-public-source-release.md)). The website and does not expose model execution, company APIs, private task records or local model files. Source publication and main promotion are handled by the public source release.

The site targets people considering owned local compute. Its time calculator uses supplied measured speeds; its ownership calculator uses entered hardware spending, API spending, watts, hours and electricity price. Blank inputs and non-finite arithmetic results stay unknown. Electricity is included; cooling, financing, maintenance, upgrades, hardware wear and frontier development are excluded. Equal task quality/volume is an explicit assumption. No GPU throughput, savings, ranking or task-completion guarantee is invented.

## Support

`/support` sends text to the owner-approved project/privacy contact, `robertjmonzingo@gmail.com`, through Resend. The configured sender uses the existing verified `robertjmonzingo.com` domain. No additional credential was needed from the owner. A dedicated Resend `sending_access` key was provisioned for this website; the existing shared credential was preserved. Support is a separately authorized website integration, not an agent tool or part of the offline $0 task path. Vercel/Resend usage depends on the owner's plans and quotas; no blanket $0 hosting/mail claim is made.

The function has separate delivery and request-policy ports and uses only Node built-ins. Credentials, recipient and sender are server-controlled. The browser cannot choose recipients or attach files. Validation limits content to 8 KiB and messages to 20–3,000 characters, checks consent and email-header injection, checks exact HTTPS origins, and uses a honeypot plus a short-lived HMAC challenge bound to the Vercel-provided connection IP. It does not log user message bodies or raw IPs.

Warm-process burst/concurrency limits, a short body deadline and delivery-replay suppression now run before expensive work. These are instance-local limits; see [fast answers and abuse limits](126-fast-answers-and-abuse-limits.md) for exact bounds and distributed-abuse limitations.

A keyed idempotency identifier limits distinct messages from one connection per UTC day across function instances using Resend's 24-hour idempotency retention. Shared networks share that limit; day boundaries can permit two nearby messages. This does not prevent distributed-IP abuse, bound all hosting traffic or guarantee zero cost. Provider timeouts are uncertain, never automatically retried. A 202 response means provider acceptance, not inbox delivery. No auto-reply is sent.

Provider documentation: [Resend idempotency](https://resend.com/docs/dashboard/emails/idempotency-keys), [Vercel trusted request headers](https://vercel.com/docs/headers/request-headers). Website privacy discloses Vercel access data, Resend delivery, Gmail mailbox storage, consent, no agent-memory/training reuse and no automatic mailbox deletion.

### Configuration

Project: `gents-office` / `prj_hOje1Hn6D2nUBFJsThaaPLRfOSt3` in **VCV Vercel Team** / `team_OW7ea1MdQ4ReouXZAgvJFElL`.

Vercel Project Settings → Environment Variables:

| Setting | Storage | Meaning |
| --- | --- | --- |
| `RESEND_API_KEY` | Secret | Dedicated sending-only mail credential; never client-public |
| `SUPPORT_SIGNING_SECRET` | Secret | At least 32 random bytes, owner-controlled |
| `SUPPORT_FROM`, `SUPPORT_TO` | Server settings | Reviewed sender and fixed recipient |
| `SUPPORT_ENABLED` | Server setting | `1` enables; `0` disables after redeployment |
| `SUPPORT_ORIGIN` | Config | Exact HTTPS origins, comma-separated; update when adding a domain |

Vercel supplies `VERCEL` and `VERCEL_URL`. The local company does not read `.env.website.local`. Its example documents website-only settings. Shared optional mail credentials remain private in `.env.local`; the dedicated website Resend key is in `.env.website.local` and Vercel Secrets. Do not upload either file. Saving Vercel variables does not update an already-created deployment: redeploy the reviewed artifact afterward.

The connector created the project/deployments but returned 403 for environment writes. The installed CLI's own authenticated login succeeded for the same explicit project/team, passing values through stdin. No credentials were put in process arguments or copied to the source tree.

## Build and review

```sh
npm run test:site
node scripts/build-introduction.mjs --vercel-preview
python3 scripts/verify_introduction.py
node scripts/build-vercel-introduction.mjs
```

Only the closed whitelist in `site/deploy` is deployable: ten public pages, public assets, build information, licensing, package/config files and one bundled support function. The packager checks regular files, exact inventory, byte hashes and matching private credential values. It does not copy the repository, `.env` files, company databases, models or development server. The artifact is ignored by Git. The private manifest is in `data/mcp-basement-verification/vercel-introduction-artifact.json`.

Use the authenticated Vercel connector to publish that exact file set with the explicit project/team. Never deploy the repository root. No automatic Git integration was installed. Verify the deployment commit separately from remote Git state. A deployment alone is not evidence of a pushed commit.

Browser checks:

```sh
# Keep this preview terminal running for local checks; stop with Ctrl+C afterward.
npm run site:preview
# In a second terminal:
npm run test:site:browser
# Inspect production without sending mail:
GENTS_SITE_URL=https://metivolve.com node scripts/introduction-browser-smoke.mjs
```

One synthetic owner support message was sent successfully during the original integration check. The retained proof records provider acceptance only. The browser script now refuses `--send-owner-support` to prevent replaying that check; routine checks submit no support request.

Desktop/mobile checks verify all ten pages, immediate setup answers, local calculator behavior, no browser storage/cookies, CSP behavior, no page errors and no unexpected origins. A separate TLS-verified byte check verifies the public files against the local deployment manifest. Private function source is never fetched as a public asset.

## Still open

The site remains an evaluation preview. Independent legal review, final release/owner trial, broader task support, media training, formal incident handling and regulated-service controls remain incomplete. The production build enables indexing and canonical-domain metadata. Search ranking must be measured from actual search tools; no SEO/AEO score or ranking guarantee is supplied.

The supplied owl brand image is preserved at `assets/brand/gents-icon-transparent.png`. Its seven-size ICO is generated by `scripts/build-brand-icon.py` and is published as `/favicon.ico`; the local office uses the same icon. Local media tools remain separate from this hosted site: see [the engine and routing boundaries](../mcp/media/README.md).
