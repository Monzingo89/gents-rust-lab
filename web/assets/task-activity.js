// Presentation of authorized host status only. No routing, tools or inference.
const uuid=value=>typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
const number=value=>typeof value==='number'&&Number.isFinite(value)&&value>=0?value:null;
const phases={queued:'Queued · waiting for host events',model_request_pending:'Model request pending',model_reply_recorded:'Model reply recorded',read_tool_pending:'Reading authorized evidence',read_tool_returned:'Evidence read returned',validation_pending:'Running authorized checks',validation_returned:'Check result recorded',plan_recorded:'Workflow plan recorded',telemetry_unavailable:'Runtime telemetry unavailable'};
const terminal={generated:'Draft generated · unverified',analyzed:'Analysis recorded',cancelled:'Request cancelled',interrupted:'Execution interrupted',needs_clarification:'Needs clarification',evidence_invalidated:'Evidence invalidated',failed:'Execution failed',timed_out:'Task deadline exceeded',blocked:'Execution blocked',requires_planning:'Planning required',answered:'Source answer recorded',insufficient_evidence:'Insufficient evidence'};

export function taskActivity(result){
 if(!uuid(result?.task_id))return null;
 const status=result.status,progress=result.progress;
 const running=status==='running';
 const phase=running?(progress?.phase||'queued'):status;
 return {task_id:result.task_id,job_id:uuid(progress?.job_id)?progress.job_id:null,status,
  label:running?(phases[phase]||phases.telemetry_unavailable):(terminal[status]||'Execution stopped'),
  phase,active:running,
  model_dispatches:number(progress?.model_dispatches),model_returns:number(progress?.model_returns),
  pending_model_requests:running?number(progress?.pending_model_requests):0,
  generative_model_calls:number(result.measurements?.generative_model_calls),
  input_tokens:number(result.measurements?.input_tokens),output_tokens:number(result.measurements?.output_tokens),
  provider_cost_usd:number(result.measurements?.provider_cost_usd)};
}

export function currentTaskActivity(value,context,now=Date.now()){
 if(!value||!context.connected||value.origin!==context.origin||value.company!==context.company)return null;
 if(now-value.observed_at_ms>15000||now<value.observed_at_ms)return {...value,active:false,stale:true,phase:'stale',label:'Status stale · check task'};
 return value;
}

export function currentTaskUsage(value,context,now=Date.now()){
 if(!value||!context.connected||value.origin!==context.origin||value.company!==context.company)return null;
 if(!Number.isFinite(value.observed_at_ms)||now-value.observed_at_ms>15000||now<value.observed_at_ms)return null;
 return value;
}

export function ceoPlanningActivity(observation,session,context,now=Date.now()){
 if(!uuid(session)||!observation)return null;
 const latest=observation.events?.filter(e=>e.run_id===session&&['ceo_planning_started','ceo_inference_requested','ceo_planning_completed','ceo_planning_abandoned'].includes(e.kind)).sort((a,b)=>a.seq-b.seq).at(-1);
 if(!latest||!['ceo_planning_started','ceo_inference_requested'].includes(latest.kind))return null;
 return currentTaskActivity({...observation,task_id:session,phase:'planning',active:true,label:'CEO planning request pending · local runtime'},context,now);
}
