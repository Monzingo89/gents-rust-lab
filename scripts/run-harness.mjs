// The direct shell entrypoint and supervised UI entrypoint must load identical
// server-side integration settings. Never print the loaded environment.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { loadCompanyRuntimeEnv } from './company-env.mjs';
import { startModelSecurityService } from './model-security-service.mjs';
import { createRuntimeBundle, bundleEnvironment, bundleExecutable, probeBundle } from './mcp-runtime-bundle.mjs';

const root = new URL('../', import.meta.url);
loadCompanyRuntimeEnv({ root: fileURLToPath(root) });
const bundle = await createRuntimeBundle(root);
probeBundle(root, bundle);
const child = spawn(bundleExecutable(root, bundle, 'gents-harness'), [], {
  cwd: root, stdio: 'inherit', env: { ...process.env, ...bundleEnvironment(bundle),
    GENTS_SQLITE_BRIDGE: bundleExecutable(root, bundle, 'gents-sqlite-bridge') },
});
const scanner = startModelSecurityService({ cwd: root });
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { scanner.stop(signal); child.kill(signal); });
child.on('error', () => { scanner.stop(); console.error('Could not start the company harness'); process.exitCode = 1; });
child.on('exit', (code, signal) => { scanner.stop(); process.exitCode = code ?? (signal ? 1 : 0); });
