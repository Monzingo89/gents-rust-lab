import test from 'node:test';
import assert from 'node:assert/strict';
import { auditCompanyEnvironment } from './check-company-env.mjs';
test('private keys never appear in the audit and do not enable paid inference',()=>{
 const result=auditCompanyEnvironment({OPENROUTER_API_KEY:'PRIVATE-DUMMY',HF_TOKEN:'PRIVATE-DUMMY'},{GENTS_LOCAL_ONLY:'1',AUTH_MODE:'local',BIND:'127.0.0.1:8788'});
 assert.deepEqual(result.required_api_keys,[]);assert.deepEqual(result.issues,[]);
 assert.equal(result.optional_credentials.HF_TOKEN.present,true);
 assert.ok(!JSON.stringify(result).includes('PRIVATE-DUMMY'));
});
test('cloud mode, exposed credentials, remote binding and invalid transport fail local audit',()=>{
 const result=auditCompanyEnvironment({VITE_PROVIDER_API_KEY:'PRIVATE-DUMMY'},{GENTS_LOCAL_ONLY:'0',BIND:'0.0.0.0:8788',GENTS_MCP_TRANSPORT:'remote'});
 assert.equal(result.issues.length,4);assert.ok(!JSON.stringify(result).includes('PRIVATE-DUMMY'));
});
