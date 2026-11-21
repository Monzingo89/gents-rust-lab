// Owner task UI over the existing authenticated task API. Model text stays text.
const known = value => value === null || value === undefined ? 'unknown' : String(value);
const label = value => String(value || 'unknown').replaceAll('_', ' ');
const date = value => {const d=new Date(value);return Number.isNaN(d.getTime())?'unknown':d.toISOString();};
export function taskSummary(result) {
 return {
  status: label(result.status), verification: label(result.verification_state),
  path: label(result.actual_path),
  usage: `Model calls: ${known(result.measurements?.generative_model_calls)} · tool calls: ${known(result.measurements?.tool_calls)} · input tokens: ${known(result.measurements?.input_tokens)} · output tokens: ${known(result.measurements?.output_tokens)} · memory bytes: ${known(result.measurements?.memory_bytes)}`,
 };
}
export function taskWorkbench({request,context,pollMs=1000}) {
 const element=document.createElement('section');element.className='mcp-floor';element.id='task-workbench';
 element.innerHTML='<h3>Answers from local evidence</h3><p>Try a cached answer or source lookup first. A configured local worker can draft an answer when synthesis is needed. Acceptance checks and source evidence are shown separately.</p><form data-task-form><label>Your question<textarea data-task-ask maxlength="12000" required></textarea></label><label>Date or period (optional)<input data-task-period maxlength="200"></label><button type="submit" data-task-answer>Get local answer</button><button type="button" data-task-analyze>Analyze only</button></form><p data-task-status role="status" aria-live="polite">Connect your company to ask a question.</p><label>Task ID<input data-task-id autocomplete="off" spellcheck="false"></label><button type="button" data-task-refresh>Check task</button><button type="button" data-task-cancel disabled>Cancel my request</button><button type="button" data-task-clear>Clear view</button><p>Clearing this view does not cancel a running task. Keep its task ID to check it later.</p><div data-task-output></div>';
 const get=name=>element.querySelector(`[data-task-${name}]`);
 let generation=0,busy=false,active=null,timer=null,requestRecord=null;
 const key=()=>{const c=context();return `${c.origin}:${c.company}`;};
 const stop=()=>{clearTimeout(timer);timer=null;};
 const controls=()=>{for(const name of ['answer','analyze'])get(name).disabled=busy||!!active;get('refresh').disabled=busy;get('cancel').disabled=busy||!active;};
 function reset(){generation++;stop();active=null;busy=false;requestRecord=null;get('id').value='';get('output').replaceChildren();get('status').textContent='Ask a question or enter a saved task ID.';controls();}
 function add(parent,tag,text){const node=document.createElement(tag);node.textContent=text;parent.append(node);return node;}
 function render(packet){
  const result=packet.result||(packet.status?packet:null),out=get('output');out.replaceChildren();
  const taskId=packet.request?.task_id||result?.task_id;if(taskId)get('id').value=taskId;
  if(packet.request)requestRecord=packet.request;
  if(requestRecord?.task_id!==taskId)requestRecord=null;
  if(requestRecord)add(out,'p',`Original question: ${requestRecord.original_ask}`);
  const plan=result?.plan||packet.proposed_plan,analysis=result?.analysis||packet.analysis;
  if(result){
   const summary=taskSummary(result);get('status').textContent=`${summary.status} · ${summary.verification}`;
   add(out,'p',`Execution path: ${summary.path}`);add(out,'p',summary.usage);
   if(result.answer!==null&&result.answer!==undefined)add(out,'pre',result.answer);
   for(const blocker of result.blockers||[])add(out,'p',`Blocked: ${blocker}`);
   for(const question of result.clarification_questions||[])add(out,'p',`Clarification needed: ${question}`);
   for(const assumption of result.assumptions||[])add(out,'p',`Assumption: ${assumption}`);
   if(result.cache){const cache=add(out,'details','');add(cache,'summary','Cache provenance');add(cache,'pre',JSON.stringify(result.cache,null,2));}
   if(result.acceptance_checks?.length){add(out,'h4','Acceptance checks');for(const check of result.acceptance_checks)add(out,'p',`${label(check.status)} · ${check.name}: ${check.detail}`);}
   if(result.evidence?.length){add(out,'h4','Source evidence');for(const source of result.evidence){const details=add(out,'details','');add(details,'summary',`${source.title} · ${source.source_revision}`);add(details,'pre',source.quote);add(details,'p',`${source.source_id} · bytes ${source.start_byte}–${source.end_byte} · SHA-256 ${source.content_sha256}`);add(details,'p',`Observed ${date(source.observed_at_ms)} · valid until ${date(source.valid_until_ms)}`);}}
  }else get('status').textContent='Analyzed · proposed plan only';
  if(plan||analysis){const details=add(out,'details','');add(details,'summary','Plan and analysis');add(details,'pre',JSON.stringify({plan,analysis},null,2));}
  active=result?.status==='running'?taskId:null;controls();
 }
 async function operation(body,{replace=false}={}){
  if(busy)return;
  if(!context().connected){get('status').textContent='Connect your company first.';return;}
  if(replace){generation++;stop();active=null;requestRecord=null;get('output').replaceChildren();get('id').value='';}
  const turn=generation,captured=key(),company=context().company;
  busy=true;controls();get('status').textContent=body.op==='status'?'Checking task…':body.op==='cancel_task'?'Cancelling your request…':'Submitting…';
  try{
   let packet=await request(`/api/companies/${encodeURIComponent(company)}/tasks`,{method:'POST',body:JSON.stringify(body)});
   if(turn!==generation||captured!==key())return;
   if(body.op==='cancel_task')packet=await request(`/api/companies/${encodeURIComponent(company)}/tasks`,{method:'POST',body:JSON.stringify({op:'status',task_id:body.task_id})});
   if(turn!==generation||captured!==key())return;
   render(packet);
   if(active){const next=active;timer=setTimeout(()=>{if(turn===generation&&captured===key())operation({op:'status',task_id:next});},pollMs);}
  }catch(error){if(turn===generation&&captured===key()){stop();get('status').textContent=`Task request unavailable: ${error.message}. Use Check task to retry a saved ID.`;}}
  finally{if(turn===generation){busy=false;controls();}}
 }
 function submit(op){if(!get('form').reportValidity())return;const input={original_ask:get('ask').value,temporal_scope:get('period').value};return operation({op,input},{replace:true});}
 get('form').onsubmit=event=>{event.preventDefault();submit('execute');};
 get('analyze').onclick=()=>submit('destructure_task');
 function taskId(){const id=get('id').value.trim();if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id))throw Error('Enter the task UUID returned by this company.');return id;}
 get('refresh').onclick=()=>{try{stop();operation({op:'status',task_id:taskId()});}catch(error){get('status').textContent=error.message;}};
 get('cancel').onclick=()=>{if(active){stop();operation({op:'cancel_task',task_id:active});}};
 get('clear').onclick=reset;
 return {element,reset};
}
