// Verify host execution coverage in the saved public admin events. Admission
// decisions have no request field and must not be mistaken for tool reviews.
export function verifyCanonicalFinalAdmin(events, analyst, writer, artifactCount, sourceTool='integer_arithmetic',authorizationMode='model_review') {
 const reviews=events.filter(e=>e.kind===(authorizationMode==='approved_workflow'?'authorization_decision':'admin_decision')).map(e=>JSON.parse(e.message)).filter(e=>e.proposal?.request?.action==='finish'&&e.proposal.request.actor===writer);
 const complete=reviews.find(e=>(authorizationMode!=='approved_workflow'||(e.authorization?.authority?.mode==='approved_workflow'&&e.authorization.model_called===false))&&e.proposal.host_verified_execution?.artifacts.passed===true&&e.proposal.host_verified_execution.artifacts.checks.length===artifactCount&&e.proposal.host_verified_execution.artifacts.checks.every(c=>c.passed===true)&&e.proposal.host_verified_execution.native_calls.checks.some(c=>c.actor===analyst&&c.tool===sourceTool&&c.passed===true));
 if(!complete)throw Error('Final admin packet omitted complete canonical execution evidence');
 return complete.proposal.host_verified_execution;
}

// A terminal failure is not an in-flight call. Missing replies still leave local
// token usage unknown; correlate by call ID without inventing token estimates.
export function modelCallAccounting(events){
 const started=events.filter(event=>event.kind==='call_started');
 const terminal=new Set(events.filter(event=>event.call_id&&['call_completed','call_failed','policy_epoch_cancelled'].includes(event.kind)).map(event=>event.call_id));
 const replied=new Set(events.filter(event=>event.call_id&&event.reply).map(event=>event.call_id));
 return {model_calls_started:started.length,failed_run_calls:events.filter(event=>event.kind==='call_failed').length,started_calls_without_reply:started.filter(event=>!event.call_id||!replied.has(event.call_id)).length,unresolved_run_calls:started.filter(event=>!event.call_id||!terminal.has(event.call_id)).length,worker_model_calls_started:started.filter(event=>event.role==='worker').length,worker_model_reply_records:events.filter(event=>event.role==='worker'&&event.reply).length,worker_model_completed_replies:events.filter(event=>event.role==='worker'&&event.kind==='call_completed'&&event.reply&&(!event.reply.local_execution||event.reply.local_execution.finish_reason==='stop')).length,native_generation_cutoffs:events.filter(event=>event.reply?.local_execution?.finish_reason==='length').length,failed_native_generation_cutoffs:events.filter(event=>event.kind==='call_failed'&&event.reply?.local_execution?.finish_reason==='length').length};
}

// Measured native-local counters, including failed cutoff receipts. Unknown
// counters must not coerce to zero; complete usage is not completed generation.
export function completedLocalUsage(usages){
 let input=0,output=0;
 for(const usage of usages){
  if(usage?.cost_usd!==0||!['input','output'].every(key=>Number.isSafeInteger(usage[key])&&usage[key]>=0))throw Error('Complete nonnegative native-local usage and zero provider fee required');
  input+=usage.input;output+=usage.output;
  if(!Number.isSafeInteger(input)||!Number.isSafeInteger(output))throw Error('Local usage total exceeds safe integer bounds');
 }
 return {local_input_tokens:input,local_output_tokens:output};
}

export function nativeReplyOutcomes(records){
 const completed=records.filter(r=>r.local_execution?.finish_reason==='stop').length;
 const cutoffs=records.filter(r=>r.local_execution?.finish_reason==='length').length;
 return {local_reply_records:records.length,completed_local_replies:completed,cutoff_local_reply_records:cutoffs,unclassified_local_reply_records:records.length-completed-cutoffs};
}

// report.elapsed_ms can cover just one continuation. Measure the public run
// separately; this includes idle/maintenance time and is not GPU compute time.
export function executionSpan(events){
 const times=events.map(e=>e.timestamp_ms);
 const valid=times.length>0&&times.every(t=>Number.isSafeInteger(t)&&t>=0);
 const monotonic=valid?times.every((t,i)=>i===0||t>=times[i-1]):null;
 return {run_observed_wall_span_ms:monotonic?times.at(-1)-times[0]:null,run_timestamps_monotonic:monotonic,run_timing_scope:'First to last public execution event; includes waits, maintenance and resumes; excludes earlier planning. Not compute time.'};
}
