import test from 'node:test';
import assert from 'node:assert/strict';
import {websiteSettings} from './sync-vercel-website-env.mjs';
const env={SUPPORT_ENABLED:'0',SUPPORT_ORIGIN:'https://metivolve.com',PAID_MODEL_KEY:'must-never-upload'};
test('only website allowlist can be uploaded',()=>{assert.equal(websiteSettings(env).PAID_MODEL_KEY,undefined);assert.equal(Object.keys(websiteSettings(env)).length,6);});
test('invalid origins and injected settings fail before any upload',()=>{
 for(const change of [{SUPPORT_ORIGIN:'http://metivolve.com'},{SUPPORT_FROM:'sender\nBcc: victim@example.com'},{SUPPORT_ENABLED:'1'}])assert.throws(()=>websiteSettings({...env,...change}));
});
