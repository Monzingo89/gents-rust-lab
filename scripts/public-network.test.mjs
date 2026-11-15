import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { EventEmitter } from 'node:events';
import { publicAddress, publicAddresses, publicUrl, policyVersion } from './public-network.mjs';
import { fetchPublic } from './browser-public-http.mjs';

const fixtures=JSON.parse(readFileSync(new URL('./fixtures/public-addresses.json',import.meta.url),'utf8'));
test('versioned admission rejects mapped, translated, private and special-purpose targets',()=>{
 assert.equal(policyVersion,1);
 for(const row of fixtures)assert.equal(publicAddress(row.address),row.allowed,row.address);
 for(const value of [null,123,{},undefined])assert.equal(publicAddress(value),false);
});
test('only public HTTPS URLs without embedded credentials are accepted',()=>{
 for(const value of ['http://example.com','file:///etc/passwd','https://user:password@example.com','https://example.com:8443'])assert.throws(()=>publicUrl(value));
 assert.equal(publicUrl('https://example.com/path?q=one').hostname,'example.com');
});
test('all DNS answers must qualify before an address can be pinned',async()=>{
 for(const address of ['127.0.0.1','::ffff:127.0.0.1','64:ff9b::7f00:1']) {
  await assert.rejects(publicAddresses('fixture.invalid',async()=>[{address:'1.1.1.1',family:4},{address,family:address.includes(':')?6:4}]));
 }
 await assert.rejects(publicAddresses('fixture.invalid',async()=>[]));
 await assert.rejects(publicAddresses('fixture.invalid',async()=>Array(65).fill({address:'1.1.1.1',family:4})));
 await assert.rejects(publicAddresses('fixture.invalid',async()=>[{address:'1.1.1.1',family:6}]));
});
function fakeRequest(observed,{status=200,body=Buffer.from('fixture')}={}){
 return (target,options,callback)=>{
  observed.push({target:target.href,options});
  const request=new EventEmitter();request.write=()=>{};request.destroy=error=>request.emit('error',error);
  request.end=()=>queueMicrotask(()=>{const response=new EventEmitter();response.statusCode=status;response.headers={location:'https://private.invalid/','content-length':String(body.length)};callback(response);response.emit('data',body);response.emit('end');});
  return request;
 };
}
test('browser transport pins the validated address and strips credential headers case insensitively',async()=>{
 const observed=[];let lookups=0;
 const result=await fetchPublic('https://public.invalid/page','GET',{Authorization:'fixture-secret',COOKIE:'fixture-cookie',Host:'private.invalid','Proxy-Authorization':'fixture-proxy',Accept:'text/html'},null,{
  lookup:async()=>{lookups++;return [{address:'1.1.1.1',family:4}];},request:fakeRequest(observed)
 });
 assert.equal(result.body.toString(),'fixture');assert.equal(lookups,1);
 assert.deepEqual(observed[0].options.headers,{Accept:'text/html'});
 observed[0].options.lookup('public.invalid',{},(error,address,family)=>{assert.equal(error,null);assert.equal(address,'1.1.1.1');assert.equal(family,4);});
 observed[0].options.lookup('public.invalid',{all:true},(error,answers)=>{assert.equal(error,null);assert.deepEqual(answers,[{address:'1.1.1.1',family:4}]);});
});
test('private destinations and private redirect hops are rejected before HTTP transport',async()=>{
 const observed=[];const request=fakeRequest(observed,{status:302});
 const redirect=await fetchPublic('https://public.invalid','GET',{},null,{lookup:async()=>[{address:'1.1.1.1',family:4}],request});
 await assert.rejects(fetchPublic(redirect.headers.location,'GET',{},null,{lookup:async()=>[{address:'::ffff:127.0.0.1',family:6}],request}));
 assert.equal(observed.length,1);
});
test('browser transport enforces its response byte limit',async()=>{
 await assert.rejects(fetchPublic('https://public.invalid','GET',{},null,{lookup:async()=>[{address:'1.1.1.1',family:4}],request:fakeRequest([],{body:Buffer.alloc(8_000_001)})}),/too large/);
});
test('browser routes use the protected transport for each request, with workers and WebSockets blocked',()=>{
 const source=readFileSync(new URL('./company-browser.mjs',import.meta.url),'utf8');
 assert.match(source,/import \{ fetchPublic \} from '\.\/browser-public-http\.mjs'/);
 assert.match(source,/context\.route\('\*\*\/\*'/);
 assert.match(source,/route\.fulfill\(await fetchPublic/);
 assert.match(source,/serviceWorkers: 'block'/);
 assert.match(source,/routeWebSocket\('\*\*\/\*'/);
});
