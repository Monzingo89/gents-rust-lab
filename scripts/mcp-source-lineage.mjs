// Model-free projection after the caller validates native receipts/authority.
// A witness proves emission of the delivered write, not causal model superiority.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const sha=value=>createHash('sha256').update(value).digest('hex');

function writes(packet){
 const proposal=Object.hasOwn(packet,'proposal')?packet.proposal:packet;
 if(!object(proposal))return [];
 const actions=Object.hasOwn(proposal,'actions')?proposal.actions:[proposal];
 if(!Array.isArray(actions)||actions.length<1||actions.length>4||!actions.every(object))return [];
 return actions.flatMap(action=>{
  if(action.action!=='files')return [];
  try{
   const args=Object.hasOwn(action,'arguments')?action.arguments:JSON.parse(action.arguments_json);
   return object(args)&&args.op==='write'?[args]:[];
  }catch{return [];}
 });
}

export function sourceLineage({events,successfulCalls,actor,path,source}){
 const writesToPath=successfulCalls.filter(c=>c.request.actor===actor&&c.request.action==='files'&&c.request.arguments.op==='write'&&c.request.arguments.path===path);
 for(const call of writesToPath)assert(typeof call.request.arguments.content==='string'&&call.result.result.result.sha256===sha(call.request.arguments.content),'Native source write hash differs from proposed bytes');
 const sourceHash=sha(source);
 assert(writesToPath.some(c=>c.request.arguments.content===source&&c.result.result.result.sha256===sourceHash),'Delivered source has no matching executed native write');
 const witnesses=events.flatMap(event=>{
  const receipt=event.reply?.local_execution;
  if(event.kind!=='call_completed'||event.role!=='worker'||event.agent!==actor||
    typeof event.model!=='string'||!event.model.startsWith('local / ')||event.model.length<=8||
    typeof event.call_id!=='string'||!event.call_id||!Number.isSafeInteger(event.seq)||event.seq<0||
    !receipt||receipt.finish_reason!=='stop'||!Number.isSafeInteger(receipt.backend_pid)||receipt.backend_pid<=0||
    !/^[a-f0-9]{64}$/.test(receipt.task_input_sha256??''))return [];
  try{
   const packet=JSON.parse(event.reply.text);
   if(!object(packet)||!writes(packet).some(args=>args.path===path&&args.content===source))return [];
   return [{actor,model:event.model,event_seq:event.seq,call_id:event.call_id,task_input_sha256:receipt.task_input_sha256}];
  }catch{return [];}
 });
 assert(witnesses.length>0,'Delivered source is absent from a completed assigned local worker write proposal');
 const deniedWrites=events.filter(event=>{
  if(event.kind!=='admin_decision')return false;
  const decision=JSON.parse(event.message);
  assert(object(decision),'Invalid host admin decision in source lineage');
  const request=decision.proposal?.request;
  return decision.model_verdict?.decision==='deny'&&request?.actor===actor&&request.action==='files'&&request.arguments?.op==='write'&&request.arguments.path===path;
 }).length;
 return {delivered_source_sha256:sourceHash,delivered_source_write_witnesses:witnesses,
  admin_denied_source_writes:deniedWrites,successful_source_writes:writesToPath.length,distinct_written_source_hashes:[...new Set(writesToPath.map(c=>sha(c.request.arguments.content)))],
  scope:'Exact delivered write emission and independently checked native write metadata; multiple witnesses do not establish which emission caused execution, model ranking, training or promotion'};
}

// Observations only. A missing/malformed event window is unknown, never zero
// post-PASS inference or authority. The caller independently verifies native audit.
export function sourceReviewPath(events,target){
 const unknown=reason=>({final_source_first_native_pass_seq:null,worker_calls_after_final_source_native_pass:null,host_compiled_rust_submissions:null,source_review_observation:{available:false,reason}});
 if(!events.every((e,i)=>Number.isSafeInteger(e.seq)&&e.seq>=0&&(!i||events[i-1].seq<e.seq)))return unknown('invalid_event_sequence');
 let rows;
 try{rows=events.filter(e=>['tool_result','frozen_contract_submission'].includes(e.kind)).map(event=>{const data=JSON.parse(event.message);assert(object(data));return {event,data};});}
 catch{return unknown('malformed_host_event');}
 const pass=rows.find(({event,data})=>{
  const result=data.result,receipt=result?.mcp_receipt,value=result?.result;
  return event.kind==='tool_result'&&data.actor===target.actor&&data.action==='rust_verify'&&data.arguments?.source_path===target.source_path&&
   result?.ok===true&&receipt?.status==='completed'&&receipt.actor===target.actor&&receipt.tool==='rust_verify'&&receipt.policy_revision===target.policy_revision&&
   value?.passed===true&&value.stage==='acceptance'&&value.source_path===target.source_path&&value.source_sha256===target.source_sha256&&value.tests_sha256===target.tests_sha256&&
   receipt.arguments_sha256===sha(canonical(data.arguments))&&receipt.result_sha256===sha(canonical(value));
 });
 if(!pass)return unknown('native_pass_event_missing');
 const compiled=rows.filter(({event,data})=>event.seq>pass.event.seq&&event.kind==='frozen_contract_submission'&&event.agent===target.actor&&
  data.origin==='host_compiled_frozen_rust_submission'&&data.authority_granted===false&&data.independent_qa_required===true&&data.model_generated_summary===false&&
  /^[a-f0-9]{64}$/.test(data.verified_gate_sha256??'')&&Array.isArray(data.rust_sources)&&data.rust_sources.some(source=>source.actor===target.actor&&source.source_path===target.source_path&&source.source_sha256===target.source_sha256&&source.tests_sha256===target.tests_sha256)).length;
 return {final_source_first_native_pass_seq:pass.event.seq,worker_calls_after_final_source_native_pass:events.filter(e=>e.seq>pass.event.seq&&e.kind==='call_started'&&e.role==='worker'&&e.agent===target.actor).length,
  host_compiled_rust_submissions:compiled,source_review_observation:{available:true,scope:'Public host-event observations after first canonical PASS for final source/tests/actor/epoch; no inference attestation, permission, QA acceptance or promotion'}};
}
