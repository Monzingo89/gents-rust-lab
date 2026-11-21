// Trusted host oracle. Candidate JavaScript runs only in the separate,
// deny-default Seatbelt browser. Never forward candidate console/errors.
import fs from 'node:fs';
import {browserProfile} from './browser_profile.mjs';
import path from 'node:path';
import http from 'node:http';
import childProcess from 'node:child_process';
import {syncBuiltinESMExports} from 'node:module';
import {pathToFileURL} from 'node:url';

const config = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const pgid = pid => Number(childProcess.execFileSync('/bin/ps', ['-p', String(pid), '-o', 'pgid='], {encoding: 'utf8'}).trim());
if (pgid(process.pid) !== process.pid) throw Error('owned_process_group_required');
let browserPid;
const nativeSpawn = childProcess.spawn;
childProcess.spawn = (command, args, options) => {
  // Playwright otherwise detaches Chromium into a group the Rust watchdog
  // would not own. This controller and driver are both host-pinned code.
  const child = nativeSpawn(command, args, {...options, detached: false});
  if (command === '/usr/bin/sandbox-exec') browserPid = child.pid;
  return child;
};
syncBuiltinESMExports();
const {chromium} = await import(pathToFileURL(path.join(config.driver, 'index.mjs')));
const assets = new Map(config.assets.map(asset => [asset.path, {body: Buffer.from(asset.text, 'utf8'), type: asset.media_type}]));
const serveAsset = (request, response) => {
  let pathname;
  try { pathname = new URL(request.url, 'http://127.0.0.1').pathname; } catch { response.writeHead(400).end(); return; }
  const asset = assets.get(pathname === '/' ? '/index.html' : pathname);
  if (request.method !== 'GET' || !asset) { response.writeHead(404).end(); return; }
  response.writeHead(200, {'Content-Type': asset.type, 'Content-Security-Policy': "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-src 'none'; worker-src 'none'; form-action 'none'", 'Cache-Control': 'no-store'});
  response.end(asset.body);
};
const server = http.createServer(serveAsset);
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
// Seatbelt's localhost filter covers both loopback families. Hold the same
// port on IPv6 as well, so the OS grant cannot reach an unrelated ::1 service.
// Failure to reserve either family stops before candidate JavaScript runs.
const server6 = http.createServer(serveAsset);
await new Promise((resolve, reject) => {server6.once('error', reject);server6.listen({port, host: '::1', ipv6Only: true}, resolve);});
const origin = `http://127.0.0.1:${port}`;
const profile = browserProfile(config.browser, config.profile, port);
let browser;
const report = {schema_version: 1, started: false, owned_process_group: false, held_loopback_families: ['ipv4', 'ipv6'], steps: [], reason: 'startup_failed', blocked_requests: 0, outer_seatbelt: true, chromium_inner_sandbox: false};
try {
  browser = await chromium.launch({
    executablePath: '/usr/bin/sandbox-exec', ignoreDefaultArgs: true,
    args: ['-p', profile, config.browser, '--no-sandbox', '--headless', '--remote-debugging-pipe', '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-breakpad', '--disable-component-update', '--disable-sync', '--disable-extensions', '--disable-gpu', '--renderer-process-limit=2', `--user-data-dir=${config.profile}`],
    env: {PATH: '/usr/bin:/bin', LC_ALL: 'C', TMPDIR: config.profile, ...(process.env.HOME ? {HOME: process.env.HOME} : {})},
    timeout: config.contract.startup_timeout_ms,
  });
  if (pgid(browserPid) !== process.pid) throw Error('browser_process_group_mismatch');
  report.started = true;
  report.owned_process_group = true;
  const context = await browser.newContext({serviceWorkers: 'block', permissions: [], acceptDownloads: false});
  await context.route('**/*', route => {
    if (new URL(route.request().url()).origin !== origin) { report.blocked_requests++; return route.abort(); }
    return route.continue();
  });
  const page = await context.newPage();
  const timer = setTimeout(() => { void context.close().catch(() => {}); }, config.contract.runtime_timeout_ms);
  timer.unref();
  await page.goto(origin, {waitUntil: 'load', timeout: config.contract.startup_timeout_ms});
  report.reason = 'steps_incomplete';
  for (const step of config.contract.cases) {
    const observation = {id: step.id, action: step.action, completed: false};
    report.steps.push(observation);
    const locator = page.locator(`css=${step.selector}`);
    if (step.action === 'expect_text') {
      const deadline = performance.now() + config.contract.step_timeout_ms;
      do {
        // Fixed expression only. Return bounded raw data to Rust in memory;
        // Rust gates privacy and performs its own exact expected comparison.
        const result = await locator.evaluate(element => {
          const text = element.textContent ?? '';
          return text.length > 8192 ? {overflow: true, text: ''} : {overflow: false, text};
        }, undefined, {timeout: Math.max(1, deadline - performance.now())});
        if (result.overflow || Buffer.byteLength(result.text) > 8192) throw Error('dom_limit');
        observation.actual = result.text;
        if (result.text === step.value) break;
        await new Promise(resolve => setTimeout(resolve, 25));
      } while (performance.now() < deadline);
      observation.completed = observation.actual === step.value;
    } else if (step.action === 'click') {
      await locator.click({timeout: config.contract.step_timeout_ms});
      observation.completed = true;
    } else if (step.action === 'fill') {
      await locator.fill(step.value, {timeout: config.contract.step_timeout_ms});
      observation.completed = true;
    } else throw Error('unsupported_step');
    if (!observation.completed) break;
  }
  clearTimeout(timer);
  report.reason = report.steps.length === config.contract.cases.length && report.steps.every(step => step.completed) ? 'completed' : 'step_failed';
} catch {
  // Browser exceptions may include candidate text or source code. Do not
  // expose or persist them, including through diagnostic stderr.
  report.reason = report.started ? 'browser_or_step_failed' : 'startup_failed';
} finally {
  if (browser) await browser.close().catch(() => {});
  server.closeAllConnections();
  server6.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await new Promise(resolve => server6.close(resolve));
}
process.stdout.write(JSON.stringify(report));
