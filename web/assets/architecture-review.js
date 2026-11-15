// One dispatched review per ID. Lost replies are recovered by reads, never POST replay.
export function reviewController({request,uuid=()=>crypto.randomUUID()}) {
 let pending=null;const terminal=status=>['ready_for_owner_review','failed','stale','review_required','missing_capability','media_plan_review_required','staffing_review_required'].includes(status);
 return {
  pending:()=>pending,
  async start({company,planning,revision,speed,days}) {
   if(pending && !pending.terminal)throw Error('Inspect the existing pending review before starting another.');
   const seconds=Number(days)*86400;
   if(!Number.isInteger(seconds)||seconds<1||seconds>30*86400)throw Error('Choose a positive budget up to 30 days.');
   const path=`/api/companies/${encodeURIComponent(company)}/planning/${encodeURIComponent(planning)}/architecture/${uuid()}`;
   pending={path,terminal:false};
   const result=await request(path,{method:'POST',body:JSON.stringify({expected_revision:revision,speed,maximum_elapsed_seconds:seconds})});
   pending.terminal=terminal(result.status);
   return result;
  },
  async inspect(){if(!pending)throw Error('No review has been started.');const result=await request(pending.path);pending.terminal=terminal(result.status);return result;},
  reset(){pending=null;}
 };
}
export function architectureReviewPanel({request,context,invalidate}) {
 const panel=document.createElement('section');panel.className='mcp-floor';panel.hidden=true;
 panel.innerHTML='<h4>Choose the local team</h4><p>The CEO recommends a work pattern and agent count. A separate local judge reviews it. This step does not approve or start a task.</p><label>Work preference<select data-speed><option value="patient">Patient · prefer sequential local work</option><option value="fast">Fast · check current hardware first</option></select></label><label>Requested time allowance (days)<input data-days type="number" min="1" max="30" step="1" value="14"></label><p data-hardware role="status">Hardware not observed yet. No speed is assumed.</p><button type="button" data-start>Ask CEO and local judge</button><button type="button" data-inspect hidden>Inspect retained review</button><p data-status role="status">Finish a CEO conversation first.</p><div data-result></div><p>Current review calls run one at a time. Fast does not enable paid services or prove a speedup. A longer time allowance cannot fix missing tools or acceptance checks.</p>';
 const get=key=>panel.querySelector(`[data-${key}]`),controller=reviewController({request});let signature='',busy=false,latest=null,preference=0;
 const present=result=>{
  latest=result;get('result').replaceChildren();get('status').textContent=result.status;
  const names={ordered:'Assembly line',adaptive_delegation:'Hub delegation',peer_review:'Peer review'};
  for(const [label,value] of [['Local CEO',result.recommendation],['Local judge',result.review]])if(value){const row=document.createElement('p');row.textContent=`${label}: ${names[value.architecture]||value.architecture} · ${value.agent_count} agent(s)${value.verdict?` · ${value.verdict}`:''}. ${value.rationale}`;get('result').append(row);}
  if(result.media_routing?.modalities){const p=document.createElement('p');p.textContent='Requested output: '+result.media_routing.modalities.join(' + ')+'. Inferred from your brief by the local judge.';get('result').append(p);}
  if(result.media_routing?.missing_tools?.length||result.media_routing?.unplanned_tools?.length){const p=document.createElement('p');p.textContent='The requested media needs an available engine and a revised CEO tool plan. No mode switch or new permission is inferred from the prompt.';get('result').append(p);}
  if(result.long_run_configuration_required){const p=document.createElement('p');p.textContent='The requested time exceeds the configured run limit. Review that configuration before starting long-running work.';get('result').append(p);}
  if(result.status==='staffing_review_required'){const p=document.createElement('p');p.textContent='The recommendation differs from the frozen staffing. Discuss the agent count with the CEO and review the new plan before preparing execution.';get('result').append(p);}
  if(result.error){const p=document.createElement('p');p.textContent=result.error;get('result').append(p);}
 };
 async function hardware(){
  const c=context();if(panel.hidden||!c.visible)return;
  try{const snapshot=await request(`/api/companies/${encodeURIComponent(c.company)}/hardware`);const devices=snapshot.accelerators?.devices??[];get('hardware').textContent=`Observed now: ${devices.map(d=>d.name).join(', ')||'GPU unknown'}; host RAM ${((snapshot.memory?.host_total_bytes??0)/1024**3).toFixed(1)} GiB. Shared Apple memory is not extra GPU RAM. Throughput and task duration remain unmeasured.`;}
  catch{get('hardware').textContent='Current hardware observation unavailable. No capacity or speed has been assumed.';}
 }
 async function act(inspect=false){if(busy)return;busy=true;get('start').disabled=true;get('inspect').disabled=true;const initial=signature,initialPreference=preference;get('status').textContent=inspect?'Reading the retained review…':'Local CEO and judge review in progress. This can take several minutes.';
  try{const c=context();const result=inspect?await controller.inspect():await controller.start({company:c.company,planning:c.session,revision:c.revision,speed:get('speed').value,days:get('days').value});if(initial===signature&&initialPreference===preference&&result.request?.speed===get('speed').value&&result.request?.maximum_elapsed_seconds===Number(get('days').value)*86400)present(result);else get('status').textContent='Earlier review retained. Preferences changed; request a fresh review.';}
  catch(error){get('status').textContent=`${error.message} Inspect the retained review; a lost reply is never automatically resent.`;}
  finally{busy=false;get('inspect').hidden=!controller.pending();get('inspect').disabled=false;get('start').disabled=!!controller.pending()&&!controller.pending().terminal;}
 }
 get('start').onclick=()=>act();get('inspect').onclick=()=>act(true);
 for(const name of ['speed','days'])get(name).onchange=()=>{preference++;latest=null;invalidate();get('result').replaceChildren();get('status').textContent='Preference changed. Observe hardware, then request a fresh review. Earlier reviews remain retained.';hardware();};
 const timer=setInterval(()=>{if(document.visibilityState==='visible')hardware();},30000);
 return {element:panel,latest:()=>latest,update(state){const c=context(),next=`${c.company}:${c.session}:${state?.revision}`;panel.hidden=state?.status!=='ready';if(next!==signature){signature=next;latest=null;controller.reset();get('result').replaceChildren();get('inspect').hidden=true;get('start').disabled=false;get('status').textContent='Ready for a local architecture recommendation.';}},destroy(){clearInterval(timer);}};
}
