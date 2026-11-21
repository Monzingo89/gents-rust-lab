import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseEnv } from 'node:util';

// Keep secrets server-side. Company-specific settings override the shared local
// file; reloads also remove settings that were deleted since the previous start.
export function createCompanyEnvLoader({ root = process.cwd(), env = process.env } = {}) {
  const original = { ...env };
  let previousKeys = [];
  return function loadCompanyEnv() {
    const values = {};
    for (const name of ['.env.local', '.env.company']) {
      const path = resolve(root, name);
      if (existsSync(path)) Object.assign(values, parseEnv(readFileSync(path, 'utf8')));
    }
    for (const key of previousKeys) {
      if (key in original) env[key] = original[key];
      else delete env[key];
    }
    Object.assign(env, values);
    previousKeys = Object.keys(values);
    return env;
  };
}

// The shell launcher owns these process settings. Local integration files may
// contain defaults for other launch modes and must not redirect this runtime.
export function loadCompanyRuntimeEnv({ root = process.cwd(), env = process.env } = {}) {
  const pinned = Object.fromEntries([
    'AUTH_MODE', 'BIND', 'CONFIG', 'RUN_STORE', 'GENTS_SUPERVISED', 'LOCAL_MODEL_PORT',
    'GENTS_LOCAL_ONLY', 'GENTS_MCP_TRANSPORT', 'HF_HUB_OFFLINE', 'HF_HUB_DISABLE_TELEMETRY', 'TRANSFORMERS_OFFLINE',
    'GENTS_RUNTIME_MANIFEST', 'GENTS_RUNTIME_MANIFEST_SHA256', 'GENTS_SQLITE_BRIDGE', 'GENTS_STARTUP_ID', 'GENTS_SUPERVISOR_PID',
    'GENTS_EMAIL_AUTO_START', 'GENTS_DISABLE_SCHEDULER', 'GENTS_OFFICE_ACTIVITY', 'GENTS_AUTO_RESUME',
  ].filter(key => key in env).map(key => [key, env[key]]));
  createCompanyEnvLoader({ root, env })();
  Object.assign(env, pinned);
  return env;
}
