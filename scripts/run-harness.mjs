// The direct shell entrypoint and supervised UI entrypoint must load identical
// server-side integration settings. Never print the loaded environment.
import { spawn } from 'node:child_process';
import { loadCompanyRuntimeEnv } from './company-env.mjs';

loadCompanyRuntimeEnv();
const child = spawn('target/debug/gents-harness', [], { stdio: 'inherit', env: process.env });
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
child.on('error', () => { console.error('Could not start the company harness'); process.exitCode = 1; });
child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
