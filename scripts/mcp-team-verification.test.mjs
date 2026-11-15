import {test} from 'node:test';
import assert from 'node:assert/strict';
import {verifyCanonicalFinalAdmin,modelCallAccounting,executionSpan,completedLocalUsage,nativeReplyOutcomes} from './mcp-team-verification.mjs';
test('known cutoff usage is retained without calling the generation completed; unknown outcomes remain explicit',()=>{
 const records=[{local_execution:{finish_reason:'stop'},usage:{input:5,output:3,cost_usd:0}},
  {local_execution:{finish_reason:'length'},usage:{input:7,output:4,cost_usd:0}}];
 assert.deepEqual(nativeReplyOutcomes(records),{local_reply_records:2,completed_local_replies:1,cutoff_local_reply_records:1,unclassified_local_reply_records:0});
 assert.deepEqual(completedLocalUsage(records.map(r=>r.usage)),{local_input_tokens:12,local_output_tokens:7});
 const unknown=nativeReplyOutcomes([...records,{local_execution:{}},{}]);assert.equal(unknown.unclassified_local_reply_records,2);assert.equal(unknown.completed_local_replies,1);
 assert.deepEqual(nativeReplyOutcomes([]),{local_reply_records:0,completed_local_replies:0,cutoff_local_reply_records:0,unclassified_local_reply_records:0});
});
test('usage proof rejects unknown, fractional, unsafe or charged counters instead of manufacturing zero',()=>{
 assert.deepEqual(completedLocalUsage([{input:12,output:3,cost_usd:0},{input:5,output:0,cost_usd:0}]),{local_input_tokens:17,local_output_tokens:3});
 assert.deepEqual(completedLocalUsage([]),{local_input_tokens:0,local_output_tokens:0});
 for(const patch of [{input:null},{input:undefined},{input:false},{input:-1},{input:1.5},{input:Infinity},{output:Number.MAX_SAFE_INTEGER+1},{cost_usd:null},{cost_usd:1}])assert.throws(()=>completedLocalUsage([{input:1,output:1,cost_usd:0,...patch}]));
 assert.throws(()=>completedLocalUsage([{input:Number.MAX_SAFE_INTEGER,output:0,cost_usd:0},{input:1,output:0,cost_usd:0}]));
});
const event=proposal=>({kind:'admin_decision',message:JSON.stringify({proposal})});
const facts=()=>({artifacts:{passed:true,checks:[{passed:true},{passed:true}]},native_calls:{checks:[{actor:'Analyst',tool:'integer_arithmetic',passed:true}]}});
const finish=host_verified_execution=>event({request:{action:'finish',actor:'Writer'},host_verified_execution});
test('admission and unrelated admin decisions cannot break or satisfy final native evidence review',()=>{
 assert.deepEqual(verifyCanonicalFinalAdmin([event({admission:true}),finish(facts())],'Analyst','Writer',2),facts());
 assert.throws(()=>verifyCanonicalFinalAdmin([event({admission:true})],'Analyst','Writer',2),/omitted/);
 for(const change of [v=>v.artifacts.checks.pop(),v=>v.artifacts.checks[0].passed=false,v=>v.native_calls.checks[0].actor='Other']){const v=facts();change(v);assert.throws(()=>verifyCanonicalFinalAdmin([finish(v)],'Analyst','Writer',2),/omitted/);}
});
test('final canonical evidence must match the actual source tool, including new native capabilities',()=>{
 const v=facts();v.native_calls.checks[0].tool='table_aggregate';
 assert.throws(()=>verifyCanonicalFinalAdmin([finish(v)],'Analyst','Writer',2),/omitted/);
 assert.deepEqual(verifyCanonicalFinalAdmin([finish(v)],'Analyst','Writer',2,'table_aggregate'),v);
});

test('terminal failed calls remain visible without being mislabeled as in flight',()=>{
 const events=[{kind:'call_started',call_id:'a',role:'worker'},{kind:'call_failed',call_id:'a',role:'worker'},{kind:'call_started',call_id:'b',role:'judge'},{kind:'call_completed',call_id:'b',role:'judge',reply:{usage:{input:10}}},{kind:'call_started',call_id:'c',role:'judge'}];
 assert.deepEqual(modelCallAccounting(events),{model_calls_started:3,failed_run_calls:1,started_calls_without_reply:2,unresolved_run_calls:1,worker_model_calls_started:1,worker_model_reply_records:0,worker_model_completed_replies:0,native_generation_cutoffs:0,failed_native_generation_cutoffs:0});
 events.push({kind:'call_failed',call_id:'c',role:'judge'});assert.equal(modelCallAccounting(events).unresolved_run_calls,0);assert.equal(modelCallAccounting(events).started_calls_without_reply,2);
 assert.equal(modelCallAccounting([{kind:'call_started',role:'worker'}]).unresolved_run_calls,1,'missing correlation is unknown, not completed');
});

test('run timing includes earlier continuations and refuses invalid or backwards clocks',()=>{
 const events=[{kind:'started',timestamp_ms:1000},{kind:'cancelled',timestamp_ms:2000},{kind:'resumed',timestamp_ms:20000},{kind:'completed',timestamp_ms:30000}];
 assert.equal(executionSpan(events).run_observed_wall_span_ms,29000);
 assert.equal(executionSpan(events).run_timestamps_monotonic,true);
 for(const input of [[],[{timestamp_ms:null}],[{timestamp_ms:NaN}],[{timestamp_ms:-1}],[{timestamp_ms:3},{timestamp_ms:2}]])assert.equal(executionSpan(input).run_observed_wall_span_ms,null);
 assert.equal(executionSpan([{timestamp_ms:3},{timestamp_ms:2}]).run_timestamps_monotonic,false);
});

test('native cutoff receipts retain call completion and failure distinctions without inventing usage',()=>{
 const events=[{kind:'call_started',call_id:'a',role:'worker'},{kind:'call_failed',call_id:'a',role:'worker',reply:{local_execution:{finish_reason:'length'},usage:{output:1}}},{kind:'call_completed',call_id:'b',role:'worker',reply:{local_execution:{finish_reason:'length'}}}];
 const result=modelCallAccounting(events);assert.equal(result.native_generation_cutoffs,2);assert.equal(result.failed_native_generation_cutoffs,1);assert.equal(result.worker_model_reply_records,2);assert.equal(result.worker_model_completed_replies,0);assert.equal(result.unresolved_run_calls,0);assert.equal(result.started_calls_without_reply,0);
});

test('approved workflow requires its explicit non-model authorization event without replacing old admin evidence',()=>{
 const native={artifacts:{passed:true,checks:[{passed:true}]},native_calls:{checks:[{actor:'Analyst',tool:'audio_inspect',passed:true}]}};
 const value={proposal:{request:{action:'finish',actor:'Writer'},host_verified_execution:native},authorization:{authority:{mode:'approved_workflow'},model_called:false}};
 const event={kind:'authorization_decision',message:JSON.stringify(value)};
 assert.deepEqual(verifyCanonicalFinalAdmin([event],'Analyst','Writer',1,'audio_inspect','approved_workflow'),native);
 assert.throws(()=>verifyCanonicalFinalAdmin([event],'Analyst','Writer',1,'audio_inspect'));
 value.authorization.model_called=true;assert.throws(()=>verifyCanonicalFinalAdmin([{...event,message:JSON.stringify(value)}],'Analyst','Writer',1,'audio_inspect','approved_workflow'));
});
