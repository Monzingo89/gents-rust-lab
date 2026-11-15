import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,copyFile,rm,symlink,chmod,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {publishComponentReview} from './mcp-component-store.mjs';
import {probeComponent} from './mcp-component-probe.mjs';
import {prepareOwnedInvocation} from './mcp-owned-invocation.mjs';
const sha=text=>createHash('sha256').update(text).digest('hex');
async function fixture(body){
 const folder=await mkdtemp(join(tmpdir(),'gents-component-owner-test-')),root=pathToFileURL(folder+'/');
 try{
  await mkdir(new URL('target/debug/',root),{recursive:true,mode:0o700});
  await copyFile(new URL('../target/debug/gents-rust-component',import.meta.url),new URL('target/debug/gents-rust-component',root));
  const source='pub fn protocol_value(x:(u64,String,Option<()>))->(u64,String,Option<()>){x}';
  const packet=await publishComponentReview(root,{'source.rs':source,'acceptance.rs':'// Protocol fixture only; no qualification.','qualification.json':'{"scope":"private owner codec fixture, not a qualified model component"}'},{source_sha256:sha(source)});
  await body(root,packet);
 }finally{
  const writable=async url=>{
   await chmod(url,0o700);
   for(const entry of await readdir(url,{withFileTypes:true}))if(entry.isDirectory())await writable(new URL(entry.name+'/',url));
  };
  await writable(root);await rm(folder,{recursive:true,force:true});
 }
}
test('private owner probe describes and snapshots the helper without activation',async()=>fixture(async(root,packet)=>{
 const d=await probeComponent(root,packet.id);
 assert.equal(d.status,'API_DESCRIPTION_PASS');assert.equal(d.model_calls,0);assert.equal(d.mcp_registered,false);assert.equal(d.result,null);
 assert.match(d.helper_sha256,/^[a-f0-9]{64}$/);
 assert.equal(d.description.source_sha256,packet.context.source_sha256);
}));
test('actual isolated owner invocation roundtrips a protocol fixture, not a benchmark implementation',{skip:process.platform!=='darwin'},async()=>fixture(async(root,packet)=>{
 const argument=['18446744073709551615','owner \\ " 😀\n',{'some':null}];
 const d=await probeComponent(root,packet.id,{function:'protocol_value',arguments:[argument]});
 assert.equal(d.status,'PRIVATE_OWNER_INVOCATION_PASS');assert.deepEqual(d.result.output,argument);
 assert.equal(d.provider_tool_fees_usd,0);assert.equal(d.model_calls,0);assert.equal(d.mcp_registered,false);
 assert.equal(d.promotion_authorized,false);
 await assert.rejects(probeComponent(root,packet.id,{function:'protocol_value();',arguments:[argument]}),/rejected/);
}));
test('source context mismatch and linked helpers are rejected before helper execution',async()=>fixture(async(root,packet)=>{
 const source='pub fn another()->u64{0}';
 const mismatched=await publishComponentReview(root,{'source.rs':source,'acceptance.rs':'// fixture','qualification.json':'{}'},{source_sha256:'0'.repeat(64)});
 await assert.rejects(probeComponent(root,mismatched.id),/source identity/);
 const binary=new URL('target/debug/gents-rust-component',root);
 await rm(binary);await symlink(new URL('../target/debug/gents-rust-component',import.meta.url),binary);
 await assert.rejects(probeComponent(root,packet.id),/owner-controlled/);
}));
test('owner invocation preparation binds exact source/API/input without granting or executing',async()=>fixture(async(root,packet)=>{
 const input=[['18446744073709551615','payload',{'some':null}]];
 const p=await prepareOwnedInvocation(root,packet.id,{id:'owner-protocol',actor:'Assigned Agent',function:'protocol_value',arguments:input});
 assert.equal(p.requirement.source_sha256,packet.context.source_sha256);
 assert.equal(p.requirement.input_sha256,sha(JSON.stringify(input)));
 assert.equal(p.source_file.sha256,p.requirement.source_sha256);
 assert.equal(p.requirement.owner_declared_review_package_id,packet.id);
 assert.equal(p.request.arguments.binding_id,p.requirement.id);
 assert.equal(p.authority_granted,false);assert.equal(p.execution_performed,false);assert.equal(p.source_write_performed,false);
 input[0][1]='changed';assert.equal(p.request.arguments.arguments[0][1],'payload');
 await assert.rejects(prepareOwnedInvocation(root,packet.id,{id:'bad/../id',actor:'Assigned Agent',function:'protocol_value',arguments:[]}));
 await assert.rejects(prepareOwnedInvocation(root,packet.id,{id:'good',actor:'Assigned Agent',function:'missing',arguments:input}));
}));
