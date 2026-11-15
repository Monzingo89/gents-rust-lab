import test from 'node:test';import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {startModelSecurityService} from './model-security-service.mjs';
test('scanner lifecycle never inherits company or provider credentials',()=>{
 let captured;const child=new EventEmitter();child.killed=false;child.kill=signal=>{child.killed=signal;};
 const service=startModelSecurityService({env:{PATH:'/usr/bin',HOME:'/private',OPENAI_API_KEY:'must-not-cross',GENTS_MCP_TOKEN:'must-not-cross'},spawnProcess:(...args)=>{captured=args;return child;}});
 assert.deepEqual(captured[1],['scripts/model_security_scan.py','--watch']);assert.deepEqual(captured[2].env,{PATH:'/usr/bin'});service.stop();assert.equal(child.killed,'SIGTERM');
});
