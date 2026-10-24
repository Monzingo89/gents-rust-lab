import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';
import { createLanHandler, privateAddress, selectLanAddress } from './mobile-access.mjs';

const origin='https://192.168.1.97:8789',code='fixture-pair-code-not-a-secret';
async function fixture(t){
 const calls=[];let clock=Date.now();
 const upstream=http.createServer(async(req,res)=>{
  let body='';for await(const chunk of req)body+=chunk;
  calls.push({path:req.url,method:req.method,body,headers:req.headers});
  res.setHeader('Content-Type','application/json');res.end(JSON.stringify({ok:true,path:req.url}));
 });upstream.listen(0,'127.0.0.1');await once(upstream,'listening');
 const target=`http://127.0.0.1:${upstream.address().port}`;
 const gateway=http.createServer(createLanHandler({origin,upstream:target,code,computer:'Fixture Mac',now:()=>clock}));gateway.listen(0,'127.0.0.1');await once(gateway,'listening');
 t.after(()=>{gateway.closeAllConnections();gateway.close();upstream.closeAllConnections();upstream.close();});
 async function request(path,{method='GET',json,headers={}}={}){
  return new Promise((resolve,reject)=>{
   const req=http.request({hostname:'127.0.0.1',port:gateway.address().port,path,method,headers:{host:new URL(origin).host,...(method!=='GET'?{origin,'content-type':'application/json'}:{}),...headers}},res=>{
    let text='';res.on('data',chunk=>text+=chunk);res.on('end',()=>resolve({status:res.statusCode,headers:res.headers,text}));
   });req.on('error',reject);req.end(json===undefined?undefined:JSON.stringify(json));
  });
 }
 async function pair(){const r=await request('/mobile/session',{method:'POST',json:{code}});assert.equal(r.status,200);return r.headers['set-cookie'][0].split(';')[0];}
 return {request,pair,calls,advance:ms=>clock+=ms};
}
test('address selection rejects public, wildcard, stale and VPN interfaces',()=>{
 for(const address of ['0.0.0.0','8.8.8.8','100.64.0.4','192.168.999.1','::1','169.254.1.2'])assert.equal(privateAddress(address),false);
 for(const address of ['192.168.1.2','10.0.0.5','172.16.1.2','::ffff:192.168.1.3'])assert.equal(privateAddress(address),true);
 const interfaces={utun1:[{family:'IPv4',address:'10.9.0.1'}],en0:[{family:'IPv4',address:'192.168.1.5'}]};
 assert.equal(selectLanAddress(undefined,interfaces),'192.168.1.5');
 assert.throws(()=>selectLanAddress('0.0.0.0',interfaces));assert.throws(()=>selectLanAddress('192.168.1.6',interfaces));
 assert.throws(()=>selectLanAddress(undefined,{utun1:interfaces.utun1}));
});
test('unpaired clients cannot see config, logs, artwork or mutate company state',async t=>{
 const f=await fixture(t);
 const login=await f.request('/');assert.equal(login.status,200);assert.match(login.text,/Pairing code/);assert.ok(!login.text.includes(code));
 for(const path of ['/api/config','/api/runs','/assets/gents/fox.jpg','/mobile/status'])assert.equal((await f.request(path)).status,401);
 assert.equal((await f.request('/api/runs',{method:'POST',json:{task:'not authorized'}})).status,401);
 assert.equal(f.calls.length,0);
});
test('pairing requires the exact origin and yields secure session cookies',async t=>{
 const f=await fixture(t);
 assert.equal((await f.request('/mobile/session',{method:'POST',json:{code},headers:{origin:'https://evil.example'}})).status,403);
 assert.equal((await f.request('/mobile/session',{method:'POST',json:{code},headers:{host:'evil.example'}})).status,403);
 assert.equal((await f.request('/mobile/session',{method:'POST',json:{code},headers:{'content-type':'text/plain'}})).status,415);
 const paired=await f.request('/mobile/session',{method:'POST',json:{code}});
 assert.equal(paired.status,200);assert.match(paired.headers['set-cookie'][0],/Secure; HttpOnly; SameSite=Strict; Path=\//);
 assert.equal(f.calls.length,0);
});
test('paired API requests target only the laptop, preserve idempotency and strip phone credentials',async t=>{
 const f=await fixture(t),cookie=await f.pair();
 const result=await f.request('/api/runs',{method:'POST',json:{task:'Fixture task'},headers:{cookie,'idempotency-key':'fixture-once',authorization:'fixture-phone-token','x-forwarded-host':'evil.example'}});
 assert.equal(result.status,200);assert.equal(f.calls.length,1);
 assert.equal(f.calls[0].path,'/api/runs');assert.equal(f.calls[0].headers['idempotency-key'],'fixture-once');
 assert.equal(f.calls[0].headers.cookie,undefined);assert.equal(f.calls[0].headers.authorization,undefined);assert.equal(f.calls[0].headers.origin,undefined);assert.equal(f.calls[0].headers['x-forwarded-host'],undefined);
 assert.deepEqual(JSON.parse(f.calls[0].body),{task:'Fixture task'});
 assert.equal((await f.request('/api/runs',{method:'DELETE',headers:{cookie,origin:'https://evil.example'}})).status,403);
 assert.equal((await f.request('/api/runs',{method:'POST',json:{},headers:{cookie,'sec-fetch-site':'cross-site'}})).status,403);
 assert.equal(f.calls.length,1);
 const status=await f.request('/mobile/status',{headers:{cookie}});assert.equal(JSON.parse(status.text).execution,'laptop');
 assert.equal((await f.request('/.env.company',{headers:{cookie}})).status,404);
 assert.equal((await f.request('https://evil.example/api/runs',{headers:{cookie}})).status,400);
});
test('incorrect codes are rate limited and do not reach the laptop',async t=>{
 const f=await fixture(t);
 for(let n=0;n<5;n++)assert.equal((await f.request('/mobile/session',{method:'POST',json:{code:'wrong'}})).status,401);
 assert.equal((await f.request('/mobile/session',{method:'POST',json:{code}})).status,429);
 f.advance(600_001);assert.equal((await f.request('/mobile/session',{method:'POST',json:{code}})).status,200);
 assert.equal(f.calls.length,0);
});
test('disconnect and expiry revoke access without canceling laptop jobs',async t=>{
 const f=await fixture(t),cookie=await f.pair();
 assert.equal((await f.request('/api/runs',{headers:{cookie}})).status,200);
 assert.equal((await f.request('/mobile/session',{method:'DELETE',headers:{cookie}})).status,200);
 assert.equal((await f.request('/api/runs',{headers:{cookie}})).status,401);
 const other=await f.pair();f.advance(7*86400_000+1);
 assert.equal((await f.request('/api/runs',{headers:{cookie:other}})).status,401);
 assert.equal(f.calls.filter(c=>c.method!=='GET').length,0);
});
test('gateway cannot be configured as an open proxy or plaintext remote endpoint',()=>{
 assert.throws(()=>createLanHandler({origin,upstream:'https://example.com',code}));
 assert.throws(()=>createLanHandler({origin:'http://192.168.1.97',code}));
});
