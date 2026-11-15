# Public source and website release operations

Metivolve is an open-source local AI company project, licensed MIT. Operators
supply their own model weights and review model licenses. The website explains
the repository and publishes its MCP cookbook; no local execution daemon, model,
private task record or training dataset is hosted there.

## Reproduce the checks

```sh
npm ci
bash scripts/check.sh
npm run test:site
npm run site:build:production
python3 scripts/verify_introduction.py
python3 scripts/audit-desktop-dependencies.py
npm audit --audit-level=low
cargo audit --deny warnings
```

The optional desktop GTK 3 graph uses exact, retained security backports. Its
audit script verifies every retained source hash before accepting the single
GLib advisory version exception. See [the patch explanation](../src-tauri/vendor/README.md).
Do not claim zero warnings from an unmodified desktop lockfile audit.

Before publication, stage only reviewed source, run `npm run release:audit` with
Gitleaks installed, and scan reachable history with `gitleaks git --redact`.
The indexed-source scanner rejects model/data/env artifacts and matching local
credential values; its generic-pattern scan has six individually reviewed public literals on exact source paths
in `.gitleaks.toml`. A clean scan reduces exposure risk; it cannot prove
that all possible credentials or vulnerabilities are absent.

## Publish only the website artifact

```sh
npm run site:build:production
python3 scripts/verify_introduction.py
node scripts/build-vercel-introduction.mjs
```

Only `site/deploy` is deployable. The builder has a closed file whitelist and scans
for matching private credentials. Never run a repository-root deployment. Local
preview builds remain noindex; production enables indexing at metivolve.com with
canonical URLs, a sitemap, crawlable content, truthful structured metadata and the
MCP cookbook. The hero uses a smaller WebP image; its original artwork remains in
the repository. There are no tracking scripts, paid model calls or public tool APIs.

Keep `npm run site:preview` running in one terminal, then run
`npm run test:site:browser` in another. Production browser verification uses
`GENTS_SITE_URL=https://metivolve.com node scripts/introduction-browser-smoke.mjs`.
Routine checks send no support message. Verify live bytes, indexing headers,
canonical URLs, sitemap, desktop/mobile behavior and Git SHA separately.

## Website secrets without copying values into commands

Configure `.env.website.local` from its committed example. Keep local inference
and unrelated service credentials in their existing ignored files.

```sh
npm run site:env:check
npm run site:env:sync
```

The first command validates settings without uploading. The second uses the
existing authenticated Vercel CLI, an explicit project/team/production target,
and stdin. It uploads only six allowlisted website settings. `RESEND_API_KEY` and
`SUPPORT_SIGNING_SECRET` are Secrets; other settings are Config. Values and provider
output are suppressed. Changes apply after a reviewed redeployment. Use a Resend
sending-only credential and the fixed owner-approved recipient. Do not send keys
in chat or commit them. Secret sync does not deploy source or alter local agents.

## Discovery and measurement

The technical foundation is crawlable, useful static content with a coherent
canonical domain, descriptive titles, internal links and structured data matching
visible claims. The public repository provides source, setup, limitations, exact
tool schemas and contribution instructions. No fake ratings, keyword stuffing,
paid mentions or invented SEO/AI score is used.

Google's [AI search guidance](https://developers.google.com/search/docs/appearance/ai-features)
says core search practices apply; no special AI markup or AI text file is required.
A [sitemap](https://developers.google.com/search/docs/crawling-indexing/sitemaps/build-sitemap)
helps discovery but does not guarantee crawling, indexing or ranking.
After release, verify metivolve.com in Google Search Console and Bing Webmaster
Tools using the owner's accounts, submit `https://metivolve.com/sitemap.xml`, and
monitor actual impressions, indexed pages, queries and crawl errors. Those account
verifications and search rankings are not established by a successful deployment.

## Source checkpoint and next work

Preserve `features/session-1`, merge the tested source into `main`, then create
`session-2` from the published main revision. Keep models, private runtime data,
secrets, generated artifacts and installed environments excluded on every branch.
The model-improvement work remains incomplete: narrow gains and rejected
candidates are documented in [the capability scorecard](131-model-capability-scorecard.md).
