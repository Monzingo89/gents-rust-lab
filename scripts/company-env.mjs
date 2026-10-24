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
