import {createHash} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';

// Project metadata only. Prompts, replies, tool arguments and error bodies can
// contain private material; their contents belong in the authenticated UI.
const label = value => typeof value === 'string' ? value.slice(0, 160).replace(/[^a-zA-Z0-9 ._:/()-]/g, '?') : undefined;
const digest = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const inference = receipt => receipt && Object.fromEntries(['backend_pid','dispatched_at_ms','received_at_ms','finish_reason','cached'].filter(k=>k in receipt).map(k=>[k,typeof receipt[k]==='string'?label(receipt[k]):receipt[k]]));
export function projectEvent(event, audit = false) {
 const detail = audit ? event.detail || {} : event;
 const receipt = detail.local_execution || detail.reply?.local_execution;
 const result = {time_ms:event.timestamp_ms, task_id:event.run_id, seq:event.seq,
  event:label(event.kind), agent:audit && event.kind.startsWith('ceo_') ? 'CEO' : label(event.agent),
  model:label(detail.model), status:label(detail.status), call_id:event.call_id};
 if(detail.inference_failure)result.inference_failure=Object.fromEntries(['code','inference_dispatched','prompt_tokens','available_tokens','context_tokens','generation_tokens'].filter(k=>k in detail.inference_failure).map(k=>[k,detail.inference_failure[k]]));
 if(Array.isArray(detail.calls))result.call_snapshots=detail.calls.map(call=>({agent:label(call.agent),model:label(call.model),requested_at_ms:call.requested_at_ms,inference:inference(call.inference)}));
 if (receipt) result.inference = inference(receipt);
 if (detail.message) result.content_sha256 = digest(detail.message);
 if (detail.reply) result.result_sha256 = digest(detail.reply);
 if (event.kind === 'tool_started' || event.kind === 'tool_result') {
  try {const tool=JSON.parse(event.message);result.tool=label(tool.action);const ok=tool.ok??tool.result?.ok;if(typeof ok==='boolean')result.ok=ok;} catch { /* Hash above still identifies the actual event. */ }
 }
 return result;
}

export async function followCompany({origin,company,signal,write=console.log,pollMs=750}) {
 let auditCursor=null, unavailable=false,seeded=false;
 const runs=new Map();
 const get=async (path,options={})=>{
  const response=await fetch(new URL(path,origin),{...options,signal:AbortSignal.any([signal,AbortSignal.timeout(5000)])});
  if(!response.ok)throw new Error('event endpoint unavailable');
  return response.json();
 };
 while(!signal.aborted){
  try{
   const state=await get(`/api/companies/${encodeURIComponent(company)}`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({op:'status'})});
   const audits=[...(state.events||[])].sort((a,b)=>a.seq-b.seq);
   if(auditCursor===null)auditCursor=audits.at(-1)?.seq||0;
   else {
    if(audits.length&&audits[0].seq>auditCursor+1)write(JSON.stringify({event:'audit_gap',after_seq:auditCursor}));
    for(const event of audits)if(event.seq>auditCursor){write(JSON.stringify(projectEvent(event,true)));auditCursor=event.seq;}
   }
   const history=await get('/api/runs');
   for(const run of history.runs||[]){
    if(run.company_id!==company)continue;
    if(runs.get(run.id)?.updated===run.updated_at_ms)continue;
    const snapshot=await get(`/api/runs/${encodeURIComponent(run.id)}`),previous=runs.get(run.id);
    const events=snapshot.events||[];
    if(previous||seeded){for(const event of events)if(event.seq>(previous?.seq??-1))write(JSON.stringify(projectEvent(event)));}
    else write(JSON.stringify({event:'task_status',task_id:run.id,status:label(run.status)}));
    runs.set(run.id,{updated:run.updated_at_ms,seq:events.at(-1)?.seq??-1});
   }
   seeded=true;
   if(unavailable)write(JSON.stringify({event:'event_stream_reconnected'}));
   unavailable=false;
  }catch{if(signal.aborted)break;if(!unavailable)write(JSON.stringify({event:'event_stream_unavailable',detail:'Retrying; inspect the office for details.'}));unavailable=true;}
  await delay(pollMs,undefined,{signal}).catch(()=>{});
 }
}
