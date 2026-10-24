import { chromium } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import dns from 'node:dns/promises';
import https from 'node:https';
import net from 'node:net';

const inputPath = process.argv[2];
const request = JSON.parse(await readFile(inputPath, 'utf8'));
const blocked = new net.BlockList();
for (const [ip, prefix] of [['0.0.0.0',8],['10.0.0.0',8],['100.64.0.0',10],['127.0.0.0',8],['169.254.0.0',16],['172.16.0.0',12],['192.0.0.0',24],['192.0.2.0',24],['192.168.0.0',16],['198.18.0.0',15],['198.51.100.0',24],['203.0.113.0',24],['224.0.0.0',3]]) blocked.addSubnet(ip, prefix, 'ipv4');
for (const [ip, prefix] of [['::',96],['64:ff9b::',96],['100::',64],['2001:db8::',32],['fc00::',7],['fe80::',10],['ff00::',8]]) blocked.addSubnet(ip, prefix, 'ipv6');

async function fetchPublic(url, method, headers, body) {
  const target = new URL(url);
  if (target.protocol !== 'https:' || target.username || target.password || (target.port && target.port !== '443')) throw new Error('Public HTTPS required');
  const addresses = await dns.lookup(target.hostname, { all: true });
  if (!addresses.length || addresses.some(a => blocked.check(a.address, a.family === 4 ? 'ipv4' : 'ipv6'))) throw new Error('Private network blocked');
  // Chromium never resolves the request itself. Every hop is fulfilled with a
  // connection pinned to the validated address, including navigation redirects.
  return await new Promise((resolve, reject) => {
    const safeHeaders = { ...headers };
    for (const key of ['host','cookie','authorization','proxy-authorization']) delete safeHeaders[key];
    const req = https.request(target, { method, headers: safeHeaders, timeout: 30000,
      lookup: (_hostname, options, callback) => options.all?callback(null,[addresses[0]]):callback(null, addresses[0].address, addresses[0].family) }, res => {
      const chunks = []; let size = 0;
      res.on('data', chunk => { size += chunk.length; if(size > 8_000_000) req.destroy(new Error('Browser response too large')); else chunks.push(chunk); });
      res.on('end', () => {
        const responseHeaders = { ...res.headers };
        delete responseHeaders['content-encoding']; delete responseHeaders['transfer-encoding']; delete responseHeaders['content-length'];
        resolve({ status: res.statusCode, headers: responseHeaders, body: Buffer.concat(chunks) });
      });
      res.on('error', reject);
    });
    req.on('timeout', () => req.destroy(new Error('Browser request timed out')));
    req.on('error', reject);
    if(body) req.write(body);
    req.end();
  });
}

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
