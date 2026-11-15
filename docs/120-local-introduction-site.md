# Local introduction site

**Historical local-only checkpoint.** The current eight-page evaluation site is now published, with a separate authorized support form. See [current website operations](123-website-support-and-deployment.md). The details below describe the earlier seven-page static-only stage.

The introduction is a separate static site under `site/`. It has no execution API, inference provider, analytics, cookies, browser storage or form submission. The private company remains a different process and origin.

From the repository root, with Node.js and Python 3 available:

```sh
npm run site:preview
```

Keep that terminal running and open `http://127.0.0.1:4173`. Stop only this preview with Ctrl+C when finished. The script builds the seven pages into ignored `site/dist/` and serves only that directory on loopback. It does not install dependencies, launch agents, download models or deploy. If port 4173 is occupied, stop your own preview or choose another port explicitly; do not kill an unrelated listener.

The source covers the introduction, local guide, capabilities/evidence, FAQ, privacy, draft terms and security/control readiness. The generated room backdrop is decorative and explicitly labeled. A real completed-task GIF is still pending. No task execution is simulated as marketing proof.

The calculator uses supplied input-processing/generation rates and an explicit setup allowance. All device labels start with unknown speeds. It computes a scenario, not hardware benchmarks, model fit or a task-completion prediction. The observed 188-second six-call case is labeled as one bounded checkout observation. Provider/tool fees are separate from equipment, electricity and frontier engineering.

## Verification

```sh
npm run test:site
```

Checks cover scenario math and invalid/unknown rates, all packaged local links/anchors, accessible descriptions, one main heading per page, metadata, JSON-LD and matching content-security-policy hashes. `scripts/check.sh` and CI now include these checks.

For browser verification, keep the preview running in another terminal and use the already-installed Playwright Chromium:

```sh
npm run test:site:browser
```

This test does not download a browser. It verifies desktop/mobile layouts, keyboard entry, supplied-rate calculation, browser-local task preparation, no persistent data, local GET-only page requests and no page/CSP errors. Screenshots and the proof are ignored under `data/mcp-basement-verification/`. Current browser observation: 1440×1000 desktop and 390×844 mobile, seven pages, no horizontal overflow, no remote requests or POSTs. This is UI evidence, not company-task or compliance acceptance.

The CUA tab-creation connection failed during this pass. Verification used a separate installed Playwright browser process, preserving the user's Chrome profile and existing office tab. The actual screenshots were inspected; an HTTP 200 alone was not used as visual proof.

## Release and search boundaries

Local preview pages have `noindex,nofollow` and `robots.txt` disallows crawling. There is no invented production URL or canonical domain. Release requires the owner task trial, reviewed source/assets/dependencies, final operator/contact/retention/legal notices, explicit deployment authorization and the real domain. Configure canonical/Open Graph URLs, sitemap and deliberate indexing only for that approved public introduction origin. Never point them at the private company daemon.

The current source includes semantic HTML, unique titles/descriptions, visible FAQ content and matching structured data, internal navigation and responsive layouts. These do not guarantee Google ranking, rich-result eligibility, AEO scores or frontier recommendations. No hidden keywords, fake reviews or fabricated benchmark/compliance claims are used. Public analytics remains absent unless the owner later explicitly configures it with appropriate disclosure/consent and no private task data.
