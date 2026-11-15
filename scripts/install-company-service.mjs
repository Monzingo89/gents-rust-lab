// Login service keeps the local supervisor alive independently of this chat.
// Only PATH is persisted; credentials continue to load server-side from .env files.
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

if (process.platform !== 'darwin') throw new Error('This installer requires macOS launchd.');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const label = 'com.gents.local-company';
const folder = resolve(homedir(), 'Library/LaunchAgents');
const target = resolve(folder, label + '.plist');
const logs = resolve(root, 'data/service');
const xml = text => String(text).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>${label}</string>
<key>ProgramArguments</key><array><string>${xml(process.execPath)}</string><string>${xml(resolve(root, 'scripts/start-company.mjs'))}</string></array>
<key>WorkingDirectory</key><string>${xml(root)}</string>
<key>EnvironmentVariables</key><dict><key>PATH</key><string>${xml(process.env.PATH)}</string><key>GENTS_NO_OPEN</key><string>1</string></dict>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/><key>ThrottleInterval</key><integer>60</integer>
<key>StandardOutPath</key><string>${xml(resolve(logs, 'company.stdout.log'))}</string>
<key>StandardErrorPath</key><string>${xml(resolve(logs, 'company.stderr.log'))}</string>
</dict></plist>
`;
if (existsSync(target) && !readFileSync(target, 'utf8').includes(xml(resolve(root, 'scripts/start-company.mjs')))) {
  throw new Error('An existing service with this label targets another checkout; refusing to replace it.');
}
const domain = `gui/${process.getuid()}`;
if (spawnSync('launchctl', ['print', `${domain}/${label}`], { stdio: 'ignore' }).status === 0) {
  console.log('The local company service is already installed and loaded.');
  process.exit(0);
}
mkdirSync(folder, { recursive: true, mode: 0o700 });
mkdirSync(logs, { recursive: true, mode: 0o700 });
writeFileSync(target, plist, { mode: 0o600 });
for (const [command, args] of [['plutil', ['-lint', target]], ['launchctl', ['bootstrap', domain, target]]]) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error || result.status !== 0) throw new Error(`${command} failed; service was not confirmed running.`);
}
console.log(`Installed ${label}. Starts at login and restarts after exits. Logs: ${logs}`);
