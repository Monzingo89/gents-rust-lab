import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {sourceLineage,sourceReviewPath} from './mcp-source-lineage.mjs';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
// Protocol data only; no benchmark function or expected task output.
const source='owned protocol fixture',actor='Author',path='candidate.rs';
const hash=createHash('sha256').update(source).digest('hex');
const write=()=>({action:'files',arguments:{op:'write',path,content:source}});
const event=packet=>({kind:'call_completed',role:'worker',agent:actor,model:'local / installed model',call_id:'observed-call',seq:3,
 reply:{text:JSON.stringify(packet),local_execution:{finish_reason:'stop',backend_pid:123,task_input_sha256:'a'.repeat(64)}}});
const call=()=>({request:{actor,...write()},result:{result:{result:{sha256:hash}}}});
const proof=(events,successfulCalls=[call()])=>sourceLineage({events,successfulCalls,actor,path,source});

test('matches direct, assessed, queued and legacy write proposals without emitting source bytes',()=>{
 for(const packet of [write(),{assessment:{},proposal:write()},{proposal:{actions:[{action:'files',arguments:{op:'read',path}},write()]}},
  {proposal:{action:'files',arguments_json:JSON.stringify(write().arguments)}}]){
  const result=proof([event(packet)]);
  assert.equal(result.delivered_source_write_witnesses[0].model,'local / installed model');
  assert.equal(result.delivered_source_sha256,hash);
  assert.equal(result.successful_source_writes,1);
  assert(!JSON.stringify(result).includes(source));
 }
});

test('source in narrative, wrong paths, nested decoys or invalid plans cannot witness authorship',()=>{
 for(const packet of [{assessment:{content:source},proposal:{action:'finish'}},
  {proposal:{...write(),arguments:{...write().arguments,path:'elsewhere.rs'}}},
  {proposal:{action:'finish',arguments:{nested:write()}}},
  {proposal:{actions:[write(),null]}},{proposal:{actions:Array(5).fill(write())}},
  {proposal:{...write(),arguments:null,arguments_json:JSON.stringify(write().arguments)}},
  {proposal:null,backup:write()}])assert.throws(()=>proof([event(packet)]),/absent/);
});

test('requires assigned completed local worker provenance and an actual source-bound native write',()=>{
 for(const change of [e=>e.agent='Other',e=>e.role='judge',e=>e.kind='call_failed',e=>e.model='paid / model',
  e=>e.reply.local_execution.finish_reason='length',e=>delete e.reply.local_execution,
  e=>e.reply.local_execution.task_input_sha256='unknown',e=>e.reply.local_execution.backend_pid=0,
  e=>e.seq=null,e=>e.call_id='',e=>e.reply.text='invalid']){
  const e=event(write());change(e);assert.throws(()=>proof([e]),/absent/);
 }
 for(const change of [c=>c.request.actor='Other',c=>c.request.arguments.path='other.rs',
  c=>c.request.arguments.content='changed',c=>c.result.result.result.sha256='b'.repeat(64)]){
  const c=call();change(c);assert.throws(()=>proof([event(write())],[c]),/native write|source write hash/);
 }
});

test('retains multiple exact emissions and distinct native source versions without claiming causality',()=>{
 const old=call();old.request.arguments.content='earlier protocol bytes';old.result.result.result.sha256=createHash('sha256').update(old.request.arguments.content).digest('hex');
 const e=event(write()),later={...event(write()),seq:9,call_id:'second-call',model:'local / another installed model'};
 const result=proof([e,later],[old,call(),call()]);
 assert.equal(result.successful_source_writes,3);
 assert.equal(result.distinct_written_source_hashes.length,2);
 assert.equal(result.delivered_source_write_witnesses.length,2);
});

test('retains exact-actor/path denied writes so first native-check success does not erase earlier rejection',()=>{
 const decision=(who=actor,where=path,verdict='deny')=>({kind:'admin_decision',message:JSON.stringify({model_verdict:{decision:verdict},proposal:{request:{actor:who,...write(),arguments:{...write().arguments,path:where}}}})});
 const events=[event(write()),decision(),decision(),decision('Other'),decision(actor,'elsewhere.rs'),decision(actor,path,'allow'),{kind:'admin_decision',message:JSON.stringify({model_verdict:{decision:'allow'},proposal:{admission:true}})}];
 assert.equal(proof(events).admin_denied_source_writes,2);
 assert.throws(()=>proof([...events,{kind:'admin_decision',message:'invalid'}]));
 assert.throws(()=>proof([...events,{kind:'admin_decision',message:'null'}]));
});

const target={actor,path,source_path:path,source_sha256:'a'.repeat(64),tests_sha256:'b'.repeat(64),policy_revision:2};
const digest=value=>createHash('sha256').update(canonical(value)).digest('hex');
function nativePass(seq,patch={}){
 const args={source_path:path},value={passed:true,stage:'acceptance',source_path:path,source_sha256:target.source_sha256,tests_sha256:target.tests_sha256,...patch};
 return {seq,kind:'tool_result',agent:'Operations',message:JSON.stringify({actor,action:'rust_verify',arguments:args,result:{ok:true,result:value,mcp_receipt:{status:'completed',actor,tool:'rust_verify',policy_revision:2,arguments_sha256:digest(args),result_sha256:digest(value)}}})};
}
test('review path counts named-author calls after first exact-byte PASS and distinguishes host submission from inference',()=>{
 const started=(seq,agent=actor,role='worker')=>({seq,agent,role,kind:'call_started'});
 const marker={seq:6,kind:'frozen_contract_submission',agent:actor,message:JSON.stringify({origin:'host_compiled_frozen_rust_submission',model_generated_summary:false,independent_qa_required:true,authority_granted:false,verified_gate_sha256:'c'.repeat(64),rust_sources:[target]})};
 const result=sourceReviewPath([started(0),nativePass(1,{source_sha256:'d'.repeat(64)}),nativePass(2),started(3),started(4,'QA','judge'),started(5,'Other'),marker,nativePass(7)],target);
 assert.equal(result.final_source_first_native_pass_seq,2);assert.equal(result.worker_calls_after_final_source_native_pass,1);assert.equal(result.host_compiled_rust_submissions,1);
});
test('missing, malformed, stale or mismatched native events cannot manufacture zero post-PASS calls',()=>{
 const bad=nativePass(0),data=JSON.parse(bad.message);data.result.mcp_receipt.policy_revision=1;bad.message=JSON.stringify(data);
 for(const events of [[],[bad],[nativePass(0,{tests_sha256:'c'.repeat(64)})],[nativePass(0,{source_path:'elsewhere.rs'})],[{...nativePass(0),message:'invalid'}],[{...nativePass(0),seq:null}]]){
  const result=sourceReviewPath(events,target);assert.equal(result.source_review_observation.available,false);assert.equal(result.worker_calls_after_final_source_native_pass,null);assert.equal(result.host_compiled_rust_submissions,null);
 }
});
