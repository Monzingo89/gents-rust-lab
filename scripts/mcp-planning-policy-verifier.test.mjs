import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,rm,symlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {planningPolicyDigest,verifyPlanningPolicy,frozenPlanningPolicy} from './mcp-planning-policy-verifier.mjs';
const candidate=(id,reasoning=0)=>({id,model:{provider:'local',model:id,path:`models/${id}.gguf`,context_tokens:8192,...(reasoning?{local_reasoning_tokens:reasoning}:{})},sha256:(id==='a'?'a':'b').repeat(64),minimum_memory_mib:1024});
const policy={failures_before_next:2,candidates:[candidate('a'),candidate('b',512)]};
function attempt(index,failures,status='failed'){
 const c=policy.candidates[index];return {revision:failures+1,status,validation_error:status==='failed'?'invalid proposal':null,model:c.model.model,reply_token_limit:2048,planning_model_selection:{policy_sha256:planningPolicyDigest(policy),candidate_id:c.id,candidate_index:index,requested_index:index,structural_failures:failures,weight_sha256:c.sha256,weight_bytes:1024,host_memory:{effective_limit_bytes:8*1024**3},skipped:[]},local_execution:{backend_pid:123,finish_reason:'stop',observed_context_tokens:8192,generation_budget:{reasoning_tokens:c.model.local_reasoning_tokens??0,final_tokens_reserved:2048}}};
}
test('canonical selection requires the exact owner policy and native failure/role/budget evidence',()=>{
 const rows=[attempt(0,0),attempt(0,1),attempt(1,2,'ready')];
 const fact=verifyPlanningPolicy(rows,policy);assert.equal(fact.planning_model_switches,1);assert.deepEqual(fact.planning_model_candidates_used,['a','b']);
 for(const mutate of [row=>row.model='other',row=>row.planning_model_selection.weight_sha256='bad',row=>row.planning_model_selection.structural_failures=99,row=>row.planning_model_selection.policy_sha256='other',row=>row.local_execution.observed_context_tokens=4096,row=>row.local_execution.generation_budget.reasoning_tokens=0,row=>row.planning_model_selection=null]){
  const changed=structuredClone(rows);mutate(changed[2]);assert.throws(()=>verifyPlanningPolicy(changed,policy));
 }
 const cutoff=structuredClone(rows);cutoff[0].local_execution.finish_reason='length';assert.throws(()=>verifyPlanningPolicy(cutoff,policy));
 assert.throws(()=>verifyPlanningPolicy(rows,null));assert.equal(verifyPlanningPolicy([{status:'ready'}],null).planning_model_selection_mode,'fixed_admin_model');
});
test('profile identity comes from bounded private frozen bytes, not report-only model claims',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'planning-policy-proof-')),root=pathToFileURL(dir+'/'),folder=new URL('data/mcp-workflow-check/00000000-0000-4000-8000-000000000001/',root);
 try{
  await mkdir(folder,{recursive:true});const file=new URL('runtime-profile.json',folder),bytes=JSON.stringify({planning_model_policy:policy});await writeFile(file,bytes);
  const profile={frozen_path:file.pathname,sha256:createHash('sha256').update(bytes).digest('hex')};assert.deepEqual(await frozenPlanningPolicy(root,profile),policy);
  await assert.rejects(frozenPlanningPolicy(root,{...profile,sha256:'bad'}));
  await assert.rejects(frozenPlanningPolicy(root,{...profile,frozen_path:join(dir,'outside.json')}));
  await rm(file);await writeFile(new URL('actual.json',folder),bytes);await symlink('actual.json',file);await assert.rejects(frozenPlanningPolicy(root,profile));
 }finally{await rm(dir,{recursive:true,force:true});}
});
test('only a canonical owner adoption resets failures, and later errors still escalate',()=>{
 const accepted={owner_confirmed_revision:4,proposal_sha256:'a'.repeat(64),data_sha256:'b'.repeat(64),catalog_sha256:'c'.repeat(64)};
 const boundary={kind:'owned_selection_adopted',start_revision:4,proposal_sha256:accepted.proposal_sha256,data_sha256:accepted.data_sha256,catalog_sha256:accepted.catalog_sha256};
 const rows=[attempt(0,0),attempt(0,1),attempt(1,2,'selection_review'),attempt(0,0),attempt(0,1),attempt(1,2,'ready')];
 rows.forEach((row,index)=>{row.revision=index+1;row.planning_model_selection.phase_scope=index<3?{kind:'session',start_revision:1}:boundary;});
 const evidence={boundaries:[boundary],accepted};
 assert.equal(verifyPlanningPolicy(rows,policy,evidence).planning_model_switches,3);
 assert.throws(()=>verifyPlanningPolicy(rows,policy),'A claimed phase cannot replace a native adoption');
 for(const mutate of [
  e=>e.accepted.proposal_sha256='d'.repeat(64),
  e=>e.boundaries[0].start_revision=3,
  e=>e.boundaries.push(structuredClone(boundary)),
 ]){const changed=structuredClone(evidence);mutate(changed);assert.throws(()=>verifyPlanningPolicy(rows,policy,changed));}
 for(const mutate of [
  r=>r[2].status='question',
  r=>delete r[3].planning_model_selection.phase_scope,
  r=>r[4].planning_model_selection.structural_failures=0,
  r=>r[5].planning_model_selection.phase_scope={kind:'session',start_revision:1},
 ]){const changed=structuredClone(rows);mutate(changed);assert.throws(()=>verifyPlanningPolicy(changed,policy,evidence));}
});
