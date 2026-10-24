import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createCompanyEnvLoader } from './company-env.mjs';

test('local key loads and company settings override local settings', () => {
  const root = mkdtempSync(join(tmpdir(), 'gents-env-'));
  try {
    writeFileSync(join(root, '.env.local'), 'RESEND_API_KEY=fixture-not-a-secret\nGENTS_EMAIL_FROM=local@example.com\n');
    writeFileSync(join(root, '.env.company'), 'GENTS_EMAIL_FROM="Gents <company@example.com>"\n');
    const env = { KEEP: 'original' };
    createCompanyEnvLoader({ root, env })();
    assert.equal(env.RESEND_API_KEY, 'fixture-not-a-secret');
    assert.equal(env.GENTS_EMAIL_FROM, 'Gents <company@example.com>');
    assert.equal(env.KEEP, 'original');
  } finally { rmSync(root, { recursive: true }); }
});

test('reload drops removed file settings and restores original process values', () => {
  const root = mkdtempSync(join(tmpdir(), 'gents-env-'));
  try {
    writeFileSync(join(root, '.env.local'), 'RESEND_API_KEY=first-fixture\nTEMP_SETTING=yes\n');
    const env = { RESEND_API_KEY: 'original-fixture' };
    const load = createCompanyEnvLoader({ root, env });
    load();
    assert.equal(env.RESEND_API_KEY, 'first-fixture');
    writeFileSync(join(root, '.env.local'), 'RESEND_API_KEY=rotated-fixture\n');
    load();
    assert.equal(env.RESEND_API_KEY, 'rotated-fixture');
    assert.equal(env.TEMP_SETTING, undefined);
    rmSync(join(root, '.env.local'));
    load();
    assert.equal(env.RESEND_API_KEY, 'original-fixture');
  } finally { rmSync(root, { recursive: true }); }
});
