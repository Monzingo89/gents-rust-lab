import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp,mkdir,copyFile,rm,chmod,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {publishComponentReview} from './mcp-component-store.mjs';
import {probeComponent} from './mcp-component-probe.mjs';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {fixtureBytes,fixtureFileSpec} from './mcp-fixture-files.mjs';
import {materializeOwnedFixture,frozenOwnedCalls,verifyOwnedResult} from './mcp-owned-fixture.mjs';
const sha=v=>createHash('sha256').update(v).digest('hex');
test('reuse oracle checks the finite synthetic optimum independently and never enters requirements',()=>{
 const f=fixtureFromId('owned-weighted-reuse'),rows=f.sourceInputs[0].arguments.arguments[0].map(r=>r.map(BigInt));
 let best=[],score=-1n;
 const lex=(a,b)=>{for(let i=0;i<Math.min(a.length,b.length);i++)if(a[i]!==b[i])return a[i]<b[i];return a.length<b.length;};
 for(let mask=0;mask<2**rows.length;mask++){
  const indices=rows.map((_,i)=>i).filter(i=>mask&(1<<i));let valid=true;
  for(const i of indices)for(const j of indices)if(i<j&&rows[i][0]<rows[j][1]&&rows[j][0]<rows[i][1])valid=false;
  const weight=indices.reduce((sum,i)=>sum+rows[i][2],0n);
  if(valid&&(weight>score||weight===score&&lex(indices,best))){score=weight;best=indices;}
 }
 assert.deepEqual(best.map(String),f.expectedResults.get('valid-case').output.some);
 assert(!f.requirements.includes(JSON.stringify(best.map(String))));assert(!f.initial.includes('expectedResults'));
 const a=frozenOwnedCalls(f,'CEO Chosen Analyst'),b=frozenOwnedCalls(f,'Another Chosen Analyst');
 assert.equal(a.length,2);assert.equal(a[0].input_sha256,sha(canonical(f.sourceInputs[0].arguments.arguments)));
 assert.equal(a[0].source_sha256,b[0].source_sha256);assert.notEqual(a[0].actor,b[0].actor);
 assert.notEqual(a[0].input_sha256,a[1].input_sha256);
});
test('owned result verification binds semantics to exact current native observation, not compiler-version prose',()=>{
 const f=fixtureFromId('owned-weighted-reuse'),input=f.sourceInputs[0],s=frozenOwnedCalls(f,'Analyst')[0];
 const result={...s,binding_id:s.id,passed:true,stage:'invocation',output:f.expectedResults.get(input.id).output,provider_cost_usd:0,promotion_authorized:false,compiler:'synthetic metadata, not compiler attestation',isolation:'synthetic fixture only',diagnostics:{passed:true,exit_code:0,signal:null,limit:null,stdout:canonical(f.expectedResults.get(input.id).output)}};
 const row={...s,binding_id:s.id,actor:'Analyst',policy_revision:4,arguments_sha256:sha(canonical(input.arguments)),result_sha256:sha(canonical(result)),passed:1};
 const authority={policy_revision:4,owned_rust_invocations:[row]};
 verifyOwnedResult(f,input,result,authority,'Analyst');
 for(const [field,value] of [['passed',false],['api_sha256','0'.repeat(64)],['output',{some:[]}],['input_sha256','0'.repeat(64)]])assert.throws(()=>verifyOwnedResult(f,input,{...result,[field]:value},authority,'Analyst'));
 for(const [field,value] of [['actor','Other'],['policy_revision',3],['arguments_sha256','0'.repeat(64)],['result_sha256','0'.repeat(64)],['passed',0]])assert.throws(()=>verifyOwnedResult(f,input,result,{...authority,owned_rust_invocations:[{...row,[field]:value}]},'Analyst'));
 assert.throws(()=>verifyOwnedResult(f,input,result,{...authority,owned_rust_invocations:[row,row]},'Analyst'));
});
test('source hydration verifies sealed custody/API and keeps fixture digest stable; test metadata is simulated',async()=>{
 const folder=await mkdtemp(join(tmpdir(),'gents-owned-fixture-')),root=pathToFileURL(folder+'/');
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true,mode:0o700});
  await copyFile(new URL('../target/debug/gents-rust-component',import.meta.url),new URL('target/debug/gents-rust-component',root));
  const source='pub fn protocol_value(x:u64)->u64{x}',digest=sha(source);
  const packet=await publishComponentReview(root,{'source.rs':source,'acceptance.rs':'// protocol data','qualification.json':canonical({fixture_simulated:true,proof:{status:'PASS',source_is_local_model_authored:true,source_sha256:digest,provider_cost_usd:0}})},{source_sha256:digest});
  const description=(await probeComponent(root,packet.id)).description;
  const base=fixtureFromId('owned-weighted-reuse'),f={...base,files:[{path:'protocol.rs',input:'unit',sha256:digest,owned_component:packet.id}],ownedComponents:[{...base.ownedComponents[0],source_path:'protocol.rs',source_sha256:digest,api_sha256:description.api_sha256,function:'protocol_value',owner_declared_review_package_id:packet.id}]};
  const original=fixtureDigest(f),hydrated=await materializeOwnedFixture(root,f);
  assert.equal(fixtureBytes(hydrated.files[0]).toString(),source);assert.equal(fixtureDigest(hydrated),original);
  assert.deepEqual(fixtureFileSpec(hydrated.files[0]),fixtureFileSpec(f.files[0]));
  assert.throws(()=>fixtureBytes(f.files[0]));
  const wrong=structuredClone(f);wrong.ownedComponents[0].api_sha256='0'.repeat(64);await assert.rejects(materializeOwnedFixture(root,wrong));
  await assert.rejects(materializeOwnedFixture(root,{...f,files:[{...f.files[0],sha256:'0'.repeat(64)}]}));
 }finally{
  const writable=async path=>{await chmod(path,0o700);for(const entry of await readdir(path,{withFileTypes:true}))if(entry.isDirectory())await writable(new URL(entry.name+'/',path));};
  await writable(root);await rm(root,{recursive:true,force:true});
 }
});
