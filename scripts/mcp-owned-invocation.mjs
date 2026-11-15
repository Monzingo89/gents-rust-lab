// Host-only preparation for a fresh owner-approved SOW. No activation, policy,
// workspace write, model call, candidate invocation or task answer is created.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
import {readOwnedEvidence,verifyComponentPackage} from './mcp-component-store.mjs';
import {probeComponent} from './mcp-component-probe.mjs';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
export async function prepareOwnedInvocation(root,componentId,{id,actor,function:name,arguments:input}){
 assert(typeof id==='string'&&/^[a-z0-9-]{1,64}$/.test(id),'Safe invocation id required');
 assert(typeof actor==='string'&&actor.trim()&&actor.length<=200&&!/[\u0000-\u001f\u007f-\u009f]/.test(actor),'Assigned actor required');
 assert(Array.isArray(input)&&input.length<=8&&Buffer.byteLength(canonical(input))<=8192,'Bounded positional invocation data required');
 const probe=await probeComponent(root,componentId),description=probe.description;
 const fn=description.interface.functions.find(f=>f.name===name);
 assert(fn&&input.length===fn.arguments.minItems,'Exact described function and arity required');
 const source=await readOwnedEvidence(root,`data/owned-rust-components/${componentId}/source.rs`,8192,true);
 assert.equal(sha(source),description.source_sha256);
 await verifyComponentPackage(root,componentId);
 const sourcePath=`owned-${componentId}.rs`;
 return {
  requirement:{id,actor,source_path:sourcePath,source_sha256:description.source_sha256,api_sha256:description.api_sha256,
   function:name,input_sha256:sha(canonical(input)),owner_declared_review_package_id:componentId},
  source_file:{path:sourcePath,text:source.toString('utf8'),sha256:description.source_sha256},
  request:{action:'owned_rust_call',arguments:{binding_id:id,arguments:structuredClone(input)}},
  interface:fn,helper_sha256:probe.helper_sha256,
  authority_granted:false,source_write_performed:false,execution_performed:false,promotion_authorized:false,
  scope:'Candidate owner-frozen requirement plus exact source bytes and data request. Storage/interface checks only; an existing package grants no authority. Host must review/freeze/approve the complete SOW and install current actor policy, stage source bytes through its owner adapter, execute through native MCP and independently accept task results. Integer types/ranges and source/API pins are enforced by the native invocation adapter.'
 };
}
