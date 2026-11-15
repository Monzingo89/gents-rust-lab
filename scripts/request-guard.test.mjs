import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequestGuard,createDeliveryGuard,supportPreflight,readLimitedBody} from '../site/server/request-guard.mjs';
import {createSupportHandler} from '../site/server/support-handler.mjs';
import {createSupportService} from '../site/server/support-service.mjs';
const env={VERCEL:'1',VERCEL_URL:'test.vercel.app',SUPPORT_ENABLED:'1',RESEND_API_KEY:'TEST',SUPPORT_SIGNING_SECRET:'s'.repeat(40),SUPPORT_TO:'owner@example.com',SUPPORT_FROM:'test@example.com'};
const headers={'x-vercel-forwarded-for':'192.0.2.4',origin:'https://test.vercel.app','content-type':'application/json'};
test('warm admission caps bursts, total work, concurrent work and client memory without evicting existing quotas',()=>{
 let time=0;const guard=createRequestGuard({now:()=>time,perClient:2,total:3,concurrency:1,maxClients:2,windowMs:100});
 const first=guard.acquire('a');assert.equal(guard.acquire('b'),null);first();first();
 guard.acquire('a')();assert.equal(guard.acquire('a'),null);guard.acquire('b')();assert.equal(guard.acquire('c'),null);
 time=100;guard.acquire('a')();guard.acquire('b')();assert.equal(guard.acquire('c'),null);
 time=200;assert.equal(typeof guard.acquire('c'),'function');
 const defaults=createRequestGuard();for(let i=0;i<8;i++)defaults.acquire('one')();assert.equal(defaults.acquire('one'),null);
});
test('metadata rejects forged origins and unsupported/oversized requests before body access',async()=>{
 assert.equal(supportPreflight({method:'POST',headers},env),0);
 for(const [change,status]of [[{origin:'https://evil.invalid'},403],[{'x-vercel-forwarded-for':'forged,192.0.2.4'},403],[{'content-type':'text/plain'},415],[{'content-encoding':'gzip'},415],[{'content-length':'8193'},413]])
  assert.equal(supportPreflight({method:'POST',headers:{...headers,...change}},env),status);
 const handler=createSupportHandler({env,deliver:()=>assert.fail('No delivery')});
 const request={method:'POST',headers:new Headers({...headers,origin:'https://evil.invalid'}),get body(){assert.fail('Rejected request body must not be read');}};
 assert.equal((await handler.fetch(request)).status,403);
 assert.equal((await handler.fetch(new Request('https://test.vercel.app/api/support'))).status,405);
});
test('streaming limits catch missing content length, stalled bodies and release slots',async()=>{
 assert.equal(await readLimitedBody(new Request('https://x.test',{method:'POST',body:'abc'})),'abc');
 await assert.rejects(readLimitedBody(new Request('https://x.test',{method:'POST',body:'x'.repeat(8193)})),e=>e.status===413);
 let cancelled=false;const body=new ReadableStream({cancel(){cancelled=true;}});
 await assert.rejects(readLimitedBody({body},{timeoutMs:15}),e=>e.status===408);assert.equal(cancelled,true);
 const handler=createSupportHandler({env,admission:createRequestGuard({concurrency:1,perClient:2}),deliver:()=>assert.fail('No email')});
 const request=body=>new Request('https://test.vercel.app/api/support',{method:'POST',headers,body});
 assert.equal((await handler.fetch(request('not json'))).status,400);
 assert.equal((await handler.fetch(request('{"action":"challenge"}'))).status,200);
 const rejected=await handler.fetch(request('{"action":"challenge"}'));assert.equal(rejected.status,429);assert.equal(rejected.headers.get('Retry-After'),'60');
});
test('delivery ledger reserves before await, suppresses uncertainty/replay and fails closed when full',async()=>{
 let time=1000000,calls=0;const claimDelivery=createDeliveryGuard({now:()=>time,maxEntries:1,lifetimeMs:86400000});
 const service=createSupportService({env,now:()=>time,claimDelivery,deliver:async()=>{calls++;throw Error('uncertain');}});
 const call=body=>service({method:'POST',headers,body});const challenge=await call({action:'challenge'});time+=4000;
 const send={action:'send',token:challenge.body.token,email:'reader@example.com',name:'',topic:'setup',message:'A synthetic support question for the setup guide.',website:'',consent:true};
 assert.equal((await call(send)).status,502);assert.equal((await call(send)).status,429);assert.equal(calls,1);
 assert.equal(claimDelivery('other'),false);time+=86400000;assert.equal(claimDelivery('other'),true);
});
