// Explicit private-owner utility. Never invoked from model text or MCP authority.
// Stored qualification is historical evidence, not permission for a fresh task.
import assert from 'node:assert/strict';
import {parseArgs} from 'node:util';
import {pathToFileURL} from 'node:url';
import {mkdtemp,writeFile,rm,chmod} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readOwnedEvidence,verifyComponentPackage} from './mcp-component-store.mjs';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');

export async function probeComponent(root,id,invocation=null){
 const packet=await verifyComponentPackage(root,id);
 const source=await readOwnedEvidence(root,`data/owned-rust-components/${id}/source.rs`,8192,true);
 assert.equal(sha(source),packet.context.source_sha256,'Package source identity disagrees with qualification context');
 // Execute a private byte snapshot, not a mutable target/debug pathname.
 const binary=await readOwnedEvidence(root,'target/debug/gents-rust-component');
 const folder=await mkdtemp(join(tmpdir(),'gents-component-probe-'));
 const executable=join(folder,'gents-rust-component');
 try{
  await writeFile(executable,binary,{mode:0o500,flag:'wx'});await chmod(executable,0o500);
  const call=request=>{
   const result=spawnSync(executable,[],{cwd:folder,env:{...(process.env.HOME?{HOME:process.env.HOME}:{}),...(process.env.GENTS_RUSTC?{GENTS_RUSTC:process.env.GENTS_RUSTC}:{})},
    input:JSON.stringify(request),encoding:'utf8',timeout:110000,maxBuffer:32768,windowsHide:true});
   assert(!result.error&&!result.signal&&result.status===0,'Component helper rejected input or failed isolated execution');
   return JSON.parse(result.stdout);
  };
  const description=call({op:'describe',source:source.toString('utf8')});
  assert.equal(description.source_sha256,packet.context.source_sha256);
  let result=null;
  if(invocation){
   assert.equal(typeof invocation.function,'string');assert(Array.isArray(invocation.arguments));
   result=call({op:'invoke',source:source.toString('utf8'),source_sha256:description.source_sha256,api_sha256:description.api_sha256,
    function:invocation.function,arguments:invocation.arguments});
   assert.equal(result.passed,true);assert.equal(result.source_sha256,description.source_sha256);assert.equal(result.api_sha256,description.api_sha256);
  }
  await verifyComponentPackage(root,id);
  return {status:invocation?'PRIVATE_OWNER_INVOCATION_PASS':'API_DESCRIPTION_PASS',component_id:id,helper_sha256:sha(binary),description,result,
   model_calls:0,provider_tool_fees_usd:0,mcp_registered:false,promotion_authorized:false,
   scope:'Explicit private owner probe of stored source through a copied owned helper. Interface/storage checks and isolated invocation only; no native MCP permission receipt, fresh task qualification, independent QA, global promotion, regulatory assurance or model learning. Current agent/run governance integration is still required before company tool use.'};
 }finally{await rm(folder,{recursive:true,force:true});}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const {values}=parseArgs({options:{component:{type:'string'},invoke:{type:'boolean'},function:{type:'string'},input:{type:'string'}}});
 assert(values.component,'--component ID required');
 assert(values.invoke?values.function&&values.input:!values.function&&!values.input,'Invocation needs --invoke --function NAME --input OWNED_RELATIVE_JSON; default only describes');
 const root=new URL('../',import.meta.url);
 const invocation=values.invoke?{function:values.function,arguments:JSON.parse(await readOwnedEvidence(root,values.input,8192))}:null;
 console.log(JSON.stringify(await probeComponent(root,values.component,invocation)));
}
