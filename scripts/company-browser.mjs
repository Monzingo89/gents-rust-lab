import { chromium } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fetchPublic } from './browser-public-http.mjs';

const inputPath = process.argv[2];
const request = JSON.parse(await readFile(inputPath, 'utf8'));

const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const context = await browser.newContext({ serviceWorkers: 'block', acceptDownloads: false, viewport: { width: 1440, height: 1000 } });
  await context.route('**/*', async route => {
    try {
      const r = route.request();
      if (['image','media','font'].includes(r.resourceType())) return route.abort();
      const headers = { ...r.headers(), 'accept-encoding': 'identity' };
      await route.fulfill(await fetchPublic(r.url(), r.method(), headers, r.postDataBuffer()));
    } catch(error) { console.error('Browser request blocked or failed: '+error.message);await route.abort(); }
  });
  await context.routeWebSocket('**/*', socket => socket.close());
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  await page.goto(request.url, { waitUntil: 'domcontentloaded', timeout: 60000 });
  for (const action of (request.actions || []).slice(0, 12)) {
    if(action.kind === 'click') await page.locator(action.selector).first().click();
    else if(action.kind === 'fill') await page.locator(action.selector).first().fill(action.value);
    else throw new Error('Unsupported browser action');
  }
  const screenshot = path.join(path.dirname(inputPath), 'browser-latest.png');
  await page.screenshot({ path: screenshot, fullPage: false });
  const result = { url: page.url(), title: await page.title(), text: (await page.locator('body').innerText()).slice(0, 20000), screenshot,
    links: await page.locator('a[href]').evaluateAll(nodes => nodes.slice(0,30).map(node => ({ text: node.textContent?.slice(0,100), url: node.href }))), untrusted_content: true };
  await writeFile(path.join(path.dirname(inputPath), 'browser-latest.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} finally { await browser.close(); }
