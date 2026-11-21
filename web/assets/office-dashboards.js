// Screen placement is image-space, independent of telemetry transport.
const definitions={
 task:{title:'SELECTED LOCAL TASK',path:['task','label'],detail:'Host journal progress for the selected company task. Pending requests and recorded replies do not establish actual inference, factual correctness or acceptance. Clearing the task view stops this display, not the task.'},
 status:{title:'COMPANY STATUS',key:'running_tasks',unit:' active tasks',path:['running_tasks'],series:'calls',detail:'Active approved tasks in this local harness journal. Room activity is presentation choreography, not proof of inference.'},
 calls:{title:'LOCAL CALLS',path:['totals','calls'],series:'calls',unit:' replies',detail:'Completed journal replies plus successful efficiency-research replies. No synthetic call counts.'},
 errors:{title:'ERRORS / FAILURES',path:['totals','errors'],series:'errors',unit:' events',detail:'Recorded call, workflow-stage and run failures. Different lifecycle events can refer to the same incident.'},
 budget:{id:'budget',title:'RECORDED PROVIDER COST',path:['totals','provider_cost_usd'],series:'provider_cost_usd',unit:' USD',detail:'Charges reported by completed replies in the retained run journal. Missing usage stays unavailable. Failed or in-flight calls and separate task stores are excluded; this display is not a spending limit or billing ledger.'},
 taskbudget:{id:'taskbudget',title:'COMPANY TASK PROVIDER COST',path:['task_usage','provider_cost_usd'],unit:' USD',detail:'Retained company task-engine receipts, counted once per job/call rather than per subscriber. Includes owner and native actor task jobs. Missing or unresolved usage stays unavailable. Legacy run journals and local research remain separate. Spending limits are enforced by the runtime ledger.'},
 gpu:{title:'LOCAL GPU',path:['hardware','gpu_percent'],series:'gpu_percent',unit:'%',detail:'Live macOS IOAccelerator device utilization, host-wide rather than per-model attribution. Missing sensors remain unavailable.'},
 handoffs:{title:'CONTEXT HANDOFFS',path:['totals','handoffs'],unit:' events',detail:'Actual retrieved/staged-context and workflow-checkpoint events in the local journal, not animation.'},
 latency:{title:'CALL LATENCY',path:['totals','average_latency_ms'],unit:' ms average',detail:'Average reported latency of completed journal and efficiency-research calls. Includes their reported loading/inference overhead.'},
 tokens:{title:'LOCAL TOKENS',path:['totals','input_tokens'],series:'tokens',unit:' input tokens',detail:'Measured input tokens from actual receipts, including local research. Output tokens and budget are shown in the expanded dashboard.'},
 research:{title:'EFFICIENCY RESEARCH',path:['research','phase'],detail:'Local, read-only evidence review. At most one 192-output-token attempt per three hours, only when aggregate evidence changes. Notes persist; model weights are not trained.'},
 memory:{title:'GPU MEMORY',path:['hardware','gpu_memory_bytes'],unit:' bytes',detail:'Host-wide accelerator in-use system memory reported by macOS, not total RAM or a per-agent allocation.'},
 overview:{title:'COMPANY OVERVIEW',path:['completed_tasks'],unit:' completed tasks',series:'calls',detail:'Actual completed jobs in this harness journal. The continuous efficiency observer is tracked separately and remains active between research windows.'},
 notebook:{title:'RESEARCH NOTEBOOK',path:['research','cycles'],unit:' local reviews',detail:'Successful native-model reviews saved privately in data/office-activity/notebook.json. Recommendations are hypotheses, not validated code improvements.'},
 inflight:{title:'IN-FLIGHT CALLS',path:['totals','inflight'],unit:' active',detail:'Unfinished journal call spans plus an active local efficiency review. Seated and working animations do not increment this gauge.'},
};
export function screenDefinitions(floor){
 if(floor===5)return [...['calls','errors','budget','gpu','handoffs','latency'].map((id,i)=>({id,...definitions[id],x:453+(i%3)*231,y:122+Math.floor(i/3)*91,w:223,h:84})),...['tokens','memory','inflight','research','status'].map((id,i)=>({id,...definitions[id],x:[232,465,1110,1260,718][i],y:[611,668,638,608,490][i],w:133,h:70}))];
 if(floor===9)return [{id:'taskbudget',...definitions.taskbudget,x:709,y:164,w:429,h:204}];
 if(floor===10)return ['overview','task','research','errors','gpu','notebook'].map((id,i)=>({id,...definitions[id],x:699+(i%3)*188,y:158+Math.floor(i/3)*115,w:178,h:103}));
 const ids=floor===11?['overview','gpu','budget']:floor===6?['research','tokens']:floor===2?['errors','handoffs']:floor===4?['gpu','memory']:['research'];
 return ids.map((id,i)=>({id,...definitions[id],x:1075,y:166+i*113,w:270,h:101}));
}
export function screenValue(screen,snapshot){
 if(screen.id==='task')return snapshot?.task?.label||'NO SELECTED TASK';
 const value=screen.path.reduce((node,key)=>node?.[key],snapshot);
 if(value===null||value===undefined||(screen.id==='taskbudget'?!snapshot?.task_usage:!snapshot?.available))return 'UNAVAILABLE';
 if(screen.id==='budget'||screen.id==='taskbudget')return `$${Number(value).toFixed(4)}`;
 if(screen.id==='memory')return `${(Number(value)/1024**3).toFixed(2)} GiB`;
 return typeof value==='number'?`${value.toLocaleString()}${screen.unit||''}`:String(value);
}
function lineChart(ctx,points,r,color){
 const values=points.map(v=>typeof v==='number'&&Number.isFinite(v)?v:null),valid=values.filter(v=>v!==null);
 if(!valid.length)return;
 const lo=Math.min(0,...valid),hi=Math.max(1,...valid);let open=false;
 ctx.beginPath();values.forEach((value,i)=>{if(value===null){open=false;return;}const x=r.x+i*r.w/Math.max(1,values.length-1),y=r.y+r.h-(value-lo)/(hi-lo)*r.h;if(open)ctx.lineTo(x,y);else ctx.moveTo(x,y);open=true;});ctx.strokeStyle=color;ctx.lineWidth=2;ctx.stroke();
}
export function paintScreens(ctx,floor,snapshot={},transform=null){
 const hits=[];
 for(const screen of screenDefinitions(floor)){
  const r=transform?transform(screen):screen;
  ctx.save();if(transform){ctx.translate(r.x,r.y);ctx.scale(r.w/screen.w,r.h/screen.h);ctx.translate(-screen.x,-screen.y);}
  const glow=ctx.createLinearGradient(screen.x,screen.y,screen.x,screen.y+screen.h);glow.addColorStop(0,'#102833');glow.addColorStop(1,'#061017');
  ctx.fillStyle=glow;ctx.fillRect(screen.x,screen.y,screen.w,screen.h);ctx.strokeStyle='#517481';ctx.lineWidth=1;ctx.strokeRect(screen.x,screen.y,screen.w,screen.h);
  ctx.textAlign='left';ctx.font='600 10px ui-sans-serif,system-ui';ctx.fillStyle='#87afa9';ctx.fillText(screen.title,screen.x+10,screen.y+16);
  const value=screenValue(screen,snapshot);ctx.fillStyle=screen.id==='errors'&&snapshot.totals?.errors?'#efaf83':'#e3f3ed';ctx.font=`500 ${screen.w>300?23:screen.id==='research'?11:15}px ui-sans-serif,system-ui`;ctx.fillText(value.slice(0,screen.w>300?40:29),screen.x+10,screen.y+39);
  if(screen.series)lineChart(ctx,(snapshot.history||[]).map(p=>p[screen.series]),{x:screen.x+10,y:screen.y+screen.h*.57,w:screen.w-20,h:screen.h*.25},'#7ad1ba');
  ctx.font='8px ui-sans-serif,system-ui';ctx.fillStyle='#668b98';ctx.fillText('MEASURED LOCAL DATA · HOVER TO EXPAND',screen.x+10,screen.y+screen.h-6);ctx.restore();
  hits.push({...screen,...r,floor});
 }
 return hits;
}

export function dashboardDetail(screen,snapshot){
 const sampled=screen.id==='taskbudget'?snapshot?.task_usage?.sampled_at_ms:snapshot?.sampled_at_ms;
 const updated=sampled?new Date(sampled).toLocaleTimeString():'Awaiting server';
 const age=sampled?Math.max(0,Math.round((Date.now()-sampled)/1000)):null;
 return {title:screen.title,value:screenValue(screen,snapshot),detail:screen.detail,updated,age,
  facts:[`Recorded provider cost: ${screenValue(definitions.budget,snapshot)}`,`Local tokens: ${snapshot?.totals?.input_tokens??'—'} input / ${snapshot?.totals?.output_tokens??'—'} output`,`${snapshot?.hardware?.source||'Awaiting GPU sensor'}`,`Research: ${snapshot?.research?.phase||'Unavailable'}`,snapshot?.totals?.usage_scope||'Usage coverage unavailable',...(snapshot?.task_usage?[`Company task provider cost: ${screenValue(definitions.taskbudget,snapshot)}`,`Company task model calls: ${snapshot.task_usage.generative_model_calls??'unknown'} · observed native replies: ${snapshot.task_usage.observed_native_replies??'unknown'}`,`Company task tokens: ${snapshot.task_usage.input_tokens??'unknown'} input / ${snapshot.task_usage.output_tokens??'unknown'} output`,`Retained task jobs: ${snapshot.task_usage.sampled_jobs??'unknown'} · fixture replies: ${snapshot.task_usage.observed_fixture_replies??'unknown'}`,`Unresolved task requests: ${snapshot.task_usage.unresolved_workflow_requests??'unknown'} · grounded jobs without receipt: ${snapshot.task_usage.grounded_jobs_without_receipt??'unknown'}`,snapshot.task_usage.coverage_complete?'Retained receipt scan complete':'Retained receipt scan limited or malformed',snapshot.task_usage.scope]:['Company task usage unavailable']),...(snapshot?.task?[`Selected task: ${snapshot.task.task_id} · ${snapshot.task.label}`,`Selected task model calls: ${snapshot.task.generative_model_calls??'unknown'} · dispatch records: ${snapshot.task.model_dispatches??'unknown'}`,`Selected task provider cost: ${snapshot.task.provider_cost_usd===null||snapshot.task.provider_cost_usd===undefined?'unknown':`$${Number(snapshot.task.provider_cost_usd).toFixed(4)}`}`]:[])],note:snapshot?.research?.latest_note?.recommendation||'No local research note yet.'};
}
