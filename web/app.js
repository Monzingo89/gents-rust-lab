import {characters,modelKey,positionKey,appearanceKey,avatarForPosition,uniqueCrewLooks,spriteSheets,spriteCell,opaqueBounds,previewPosition,spriteContains} from './gent-avatars.js';
import {renderTower} from './assets/tower-scene.js';
import {towerFloors} from './assets/tower-layout.js';
const $=id=>document.getElementById(id);
let avatarChoices={};
try{const saved=JSON.parse(localStorage.getItem('gents:unique-crew:v2')||'{}');if(saved&&typeof saved==='object'&&!Array.isArray(saved))avatarChoices=saved;}catch{}
const spriteFrames=new Map();
for(const [sheetIndex,src] of spriteSheets.entries()){
 const image=new Image();image.onload=()=>{
  const scratch=document.createElement('canvas');scratch.width=image.naturalWidth;scratch.height=image.naturalHeight;
  const context=scratch.getContext('2d',{willReadFrequently:true});context.drawImage(image,0,0);
  const {data}=context.getImageData(0,0,scratch.width,scratch.height);
  for(let cell=0;cell<4;cell++){const index=sheetIndex*4+cell; spriteFrames.set(characters[index].id,{image,data,width:scratch.width,...opaqueBounds(data,scratch.width,spriteCell(index,scratch.width,scratch.height))});}
 };image.src=src;
}
let crewLooks=new Map();
const characterFor=(id,phase=null)=>{
 const agent=phase?null:sprites.find(s=>positionKey(s.id)===positionKey(id)&&s.role==='worker')||sprites.find(s=>positionKey(s.id)===positionKey(id));
 return crewLooks.get(appearanceKey(agent?.id||id,agent?.role||phase))||avatarForPosition(id,phase,avatarChoices);
};
function assignCrewLooks(){crewLooks=uniqueCrewLooks(sprites,avatarChoices);}
// A phone's localhost is the phone, not the laptop. Always use this page's origin.
let base=['http:','https:'].includes(location.protocol)?location.origin:'http://127.0.0.1:8788';
$('base').value=base;
let config=null,runId=null,events=[],observability=null,seen=0,poll=null,token='',sprites=[],working=false,pending=null,runStarted=0,runHistory=[],activeGent=null,hoverGent=null,drawerPinned=false;
let companyId='we-need-gents-lc',companyState=null,attention=null,attentionRun=null,backendOnline=false,followCompany=true,mobileAccessMode=false,conferenceUntil=0;
const lines=id=>$(id).value.split('\n').map(x=>x.trim()).filter(Boolean);
const runKey=()=>`gents-run:${base}:${companyId}`;
const rooms={planning:[3,3],library:[7.5,3],training:[10.4,3],meeting:[3,8.5],workshop:[3,9],review:[9,9],sleep:[6,6],done:[6,6],routing:[5,2],judging:[10,9],operations:[6,6]};
const colors=['#9de3bf','#e4b4ff','#ffcf8d','#8ed6ff','#f89dbe','#e7e794','#a2a3ff','#77d8d4'];
const canvas=$('office'),ctx=canvas.getContext('2d');
let towerFilter='all',hoverFloor=null,walkingPreview=false;
const motionPreference=matchMedia('(prefers-reduced-motion: reduce)');
document.documentElement.classList.add('tower-theme');
document.querySelector('.floor-kicker').textContent='THE GENTS / VERTICAL CAMPUS';
document.querySelector('.office-title h2').textContent='A higher standard of work.';
document.querySelector('.floor-chip').hidden=true;
document.querySelector('.rotate-hint').textContent='SELECT A WINDOW BAND TO ENTER A FLOOR';
canvas.setAttribute('aria-label','Open-front eleven-floor Gents tower with distinct lobby, laboratory, conference, recharge, operations, engineering, libraries, CFO, CEO, and Codex rooms.');
const floorControl=document.createElement('nav');floorControl.className='tower-floor-controls';floorControl.setAttribute('aria-label','Skyscraper floors');
for(const [value,label]of [['all','← Building exterior'],...towerFloors.map(f=>[String(f.id),`${String(f.id).padStart(2,'0')} ${f.name}`])]){
 const button=document.createElement('button');button.type='button';button.textContent=label;button.dataset.floor=value;button.setAttribute('aria-pressed',String(value==='all'));
 button.onclick=()=>selectTowerFloor(value);floorControl.append(button);
}
canvas.parentElement.before(floorControl);
const towerSummary=document.createElement('p');towerSummary.className='tower-summary';towerSummary.id='tower-summary';towerSummary.setAttribute('aria-live','polite');floorControl.before(towerSummary);
const walkingControls=document.createElement('div');walkingControls.className='walking-controls';walkingControls.innerHTML='<button type="button" id="preview-walking" aria-pressed="false" disabled>Preview walking</button><span id="walking-note">Select a window band or floor to enter the building.</span><strong id="walking-preview-warning" hidden>VISUAL PREVIEW ONLY — no models or tasks are started.</strong>';floorControl.after(walkingControls);
function selectTowerFloor(value){
 towerFilter=String(value);walkingPreview=false;clearHover();
 floorControl.querySelectorAll('button').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.floor===towerFilter)));
 const canPreview=!['all','1','4'].includes(String(value));$('preview-walking').disabled=!canPreview;$('preview-walking').setAttribute('aria-pressed','false');$('walking-preview-warning').hidden=true;
 $('walking-note').textContent=value==='all'?'Select a window band or floor to enter the building.':String(value)==='4'?'Deep recharge gallery · every sleeping Gent has a visible pod.':String(value)==='1'?'Grand hotel lobby · Peacock Reception is always staffed.':'Deep purpose-built floor · active Gents are shown in their assigned room.';
 const zoom=value!=='all'&&matchMedia('(max-width:760px)').matches?200:100;
 $('office-zoom').value=String(zoom);$('office-zoom-value').textContent=`${zoom}%`;canvas.style.width=`${zoom}%`;
 canvas.parentElement.scrollLeft=0;document.querySelector('.rotate-hint').textContent=value==='all'?'SELECT A WINDOW BAND TO ENTER A FLOOR':'SELECT A GENT / POD TO INSPECT';
 if(value!=='all')canvas.parentElement.scrollIntoView({behavior:motionPreference.matches?'instant':'smooth',block:'start'});
 observabilityFloor.hidden=value!=='10';
}
$('preview-walking').onclick=()=>{walkingPreview=!walkingPreview;$('preview-walking').setAttribute('aria-pressed',String(walkingPreview));$('walking-preview-warning').hidden=!walkingPreview;clearHover();};
const crewManifest=document.createElement('details');crewManifest.className='crew-manifest';crewManifest.innerHTML='<summary>Crew manifest · select a Gent</summary><div class="crew-manifest-list"></div>';canvas.parentElement.after(crewManifest);
let manifestSignature='';
function updateCrewManifest(){
 const signature=sprites.map(s=>`${s.id}:${s.role}:${s.busy}:${characterFor(s.id,s.role)?.id}`).join('|');if(signature===manifestSignature)return;manifestSignature=signature;
 crewManifest.querySelector('summary').textContent=`Crew manifest · ${sprites.length} distinct Gents`;
 crewManifest.querySelector('div').replaceChildren(...sprites.map((s,i)=>{
  const look=characterFor(s.id,s.role),button=document.createElement('button');button.type='button';button.dataset.crewKey=`${s.id}:${s.role}`;button.dataset.character=look.id;
  const img=document.createElement('img');img.src=look.src;img.alt='';img.loading='lazy';img.decoding='async';
  const info=document.createElement('span'),name=document.createElement('strong'),state=document.createElement('small');name.textContent=`${s.id.replace(' Gent','')} · ${s.role}`;state.textContent=`${look.name} / ${s.busy?'At work':`Recharge pod ${String(i+1).padStart(2,'0')}`}`;
  info.append(name,state);button.append(img,info);button.onclick=()=>showGent(s.id,true,s.role);return button;
 }));
}
const zoomControl=document.createElement('label');zoomControl.className='office-zoom';
zoomControl.innerHTML='Scene scale <input id="office-zoom" type="range" min="100" max="300" step="25" value="100" aria-label="Office zoom"><output id="office-zoom-value">100%</output><span>Select a floor for a close-up. Swipe to pan; tap a Gent or sleep pod.</span>';
canvas.parentElement.before(zoomControl);
const mobileMission=document.createElement('details');mobileMission.className='mobile-mission';mobileMission.innerHTML='<summary>Current mission</summary><p id="mobile-task-text">No active job</p>';
zoomControl.before(mobileMission);
const observabilityFloor=document.createElement('section');observabilityFloor.className='observability-floor';observabilityFloor.hidden=true;observabilityFloor.innerHTML='<div class="observability-heading"><div><small>FLOOR 10 / CEO ENTERPRISE ASSURANCE</small><h3>Observability, failures, compliance, bias & drift</h3></div><span id="assurance-state">Controls enforced · certification not asserted</span></div><div id="trace-summary" class="trace-summary"></div><div class="trace-columns"><section><h4>Model calls</h4><div id="trace-calls"></div></section><section><h4>Context handoffs</h4><div id="trace-handoffs"></div></section><section><h4>Issues → feedback</h4><div id="trace-issues"></div></section></div>';
canvas.parentElement.after(observabilityFloor);
const codexPhone=document.createElement('button');codexPhone.type='button';codexPhone.className='codex-phone';codexPhone.setAttribute('aria-label','Open the local executive escalation office');codexPhone.innerHTML='<span aria-hidden="true">☎</span><strong>LOCAL EXECUTIVE</strong><small>30B first · OpenAI last</small>';codexPhone.onclick=()=>{selectTowerFloor('11');if(attention)$('attention-panel').scrollIntoView({behavior:motionPreference.matches?'instant':'smooth',block:'start'});};canvas.parentElement.append(codexPhone);
function renderObservability(){
 const trace=observability;if(!trace){$('trace-summary').textContent='Select an active or saved run to inspect its harness trace.';for(const id of ['trace-calls','trace-handoffs','trace-issues'])$(id).textContent='No trace loaded.';return;}
 const calls=trace.calls||[],handoffs=trace.context_handoffs||[],issues=trace.issues||[],feedback=trace.feedback_loops||[],classification=trace.assurance?.data_classification?.level||'standard';
 $('trace-summary').innerHTML=`<article><small>CALLS</small><strong>${calls.length}</strong></article><article><small>HANDOFFS</small><strong>${handoffs.length}</strong></article><article><small>ISSUES</small><strong>${issues.length}</strong></article><article><small>REPAIR LOOPS</small><strong>${feedback.length}</strong></article><article><small>DATA</small><strong>${esc(classification)}</strong></article>`;
 $('trace-calls').replaceChildren(...(calls.slice(-18).reverse().map(call=>{const row=document.createElement('p');row.dataset.state=call.state;row.textContent=`${call.agent} / ${call.role} · ${call.model||'system'} · ${call.state}${call.latency_ms!=null?` · ${call.latency_ms} ms`:''}`;return row;})));
 $('trace-handoffs').replaceChildren(...(handoffs.slice(-18).reverse().map(event=>{const row=document.createElement('p');let detail={};try{detail=JSON.parse(event.message);}catch{}row.textContent=`#${event.seq} ${event.agent} · ${event.kind} · ${detail.records??detail.tier??'metadata only'}${detail.tiers?` · ${JSON.stringify(detail.tiers)}`:''}`;return row;})));
 const combined=[...issues.map(event=>({event,type:'issue'})),...feedback.map(event=>({event,type:'feedback'}))].sort((a,b)=>(b.event.seq||0)-(a.event.seq||0));
 $('trace-issues').replaceChildren(...(combined.slice(0,20).map(({event,type})=>{const row=document.createElement('p');row.dataset.state=type;row.textContent=`#${event.seq} ${type.toUpperCase()} · ${event.kind} · ${event.agent}`;return row;})));
 for(const id of ['trace-calls','trace-handoffs','trace-issues'])if(!$(id).children.length)$(id).textContent='None recorded.';
}
ctx.imageSmoothingEnabled=true;
function error(e){if(e?.name==='AbortError')return;$('error').textContent=e.message||String(e);}
const pendingRequests=new Set();
async function api(path,options={}){
 const controller=new AbortController();pendingRequests.add(controller);
 try{
  const headers={'Content-Type':'application/json',...options.headers};if(token)headers.Authorization=`Bearer ${token}`;
  const r=await fetch(base+path,{...options,headers,signal:controller.signal});
  if(r.status===401&&mobileAccessMode)location.replace('/');
  if(!r.ok){const e=new Error(`${r.status}: ${await r.text()}`);e.status=r.status;throw e;}
  return await r.json();
 }finally{pendingRequests.delete(controller);}
}
function renderOrganization(){
 const organization=config?.organization;if(!organization)return;
 document.querySelector('.ops-map > summary span').textContent='Characters, models & memory';
 const lanes=[['Executive oversight',r=>r.tier===-1],['Leadership',r=>r.tier===0],['Governance & delivery',r=>r.tier===1],['Architecture & enablement',r=>r.tier===2],['Engineering & assurance',r=>r.tier>=3]];
 $('org-chart').replaceChildren(...lanes.map(([label,predicate])=>{const lane=document.createElement('div');lane.className='org-tier';const heading=document.createElement('h3');heading.textContent=label;lane.append(heading);for(const role of organization.roles.filter(predicate)){const button=document.createElement('button');button.type='button';button.className='org-card';button.dataset.group=role.group;button.innerHTML=`<strong>${esc(role.title)}</strong><small>${esc(role.scalable?'1..many · ': '')}${esc(role.split?.join(' + ')||role.group)}</small>`;button.onclick=()=>showGent(role.id,true);lane.append(button);}return lane;}));
 const tiers=organization.context?.tiers||[];$('context-flow').replaceChildren(...tiers.map(t=>{const node=document.createElement('div');node.className='context-node';node.innerHTML=`<strong>${esc(t.id)} · ${esc(t.temperature)}</strong><span>${esc(t.meaning)}</span>`;return node;}),Object.assign(document.createElement('div'),{className:'context-dream',textContent:`DREAM · ${organization.execution?.daily_dream_budget_minutes||120} MIN / DAY`}));
}
$('connect').onclick=async()=>{
 try{
  if(working)throw new Error('Finish or cancel the active view before changing connections');
  const u=new URL($('base').value);if(u.protocol!=='https:'&&!(u.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(u.hostname)))throw new Error('Use HTTPS for a remote harness.');
  base=u.origin;token=$('token').value.trim();config=await api('/api/config');backendOnline=true;renderOrganization();decorateRoster();
  $('company-controls').hidden=!config.company_mode;
  if(config.company_mode){
   companyId=$('company-id').value.trim();await refreshCompany();$('run').disabled=false;$('run').textContent='Ask For a Task';$('task').readOnly=false;$('acceptance-checks').hidden=true;
   if(!$('task').dataset.companyInput){$('task').value='';$('task').placeholder='Describe the goal, relevant details, and what a great result looks like...';$('task').dataset.companyInput='true';}
   $('consent').checked=false;$('consent').disabled=false;$('consent-text').textContent='Start the company on this request. The admin judge reviews its web, email, code, model and training actions.';
  }else{$('run').textContent='Start task';$('acceptance-checks').hidden=false;}
  $('company-name').textContent=(companyState?.name||config.company_name||'The Gents Office Co.').toUpperCase();$('status').textContent=`Connected · ${config.auth} · ${config.organization?.roles?.length||config.agents.length} role types`;
  if($('providers'))$('providers').textContent=[...new Set(config.agents.flatMap(a=>[a.planner_provider,a.worker_provider]).concat(config.judges?[config.judges.router_provider,config.judges.completion_provider]:[]).concat(config.catalog_providers||[]))].join(', ');
  if(config.poc){$('task').value=config.poc.task;$('task').readOnly=true;$('dod').value=config.poc.criteria.join('\n');$('dod').readOnly=true;$('required').disabled=true;$('forbidden').disabled=true;$('consent').checked=true;$('consent').disabled=true;$('consent-text').textContent='All inference stays on this Mac; no cloud provider is used.';}
  sprites=config.agents.flatMap((a,i)=>['planner','worker'].map((role,j)=>({id:a.id,role,color:colors[i%colors.length],x:5.25+i*1.2,y:5.35+j*.85,tx:5.25+i*1.2,ty:5.35+j*.85,waiting:false,index:i,hit:null})));
  const homes=[[1.5,3.8],[3.7,3.7],[1.9,10.8],[4.2,10.5],[3,7.8],[9,10.7]];
  for(const sprite of sprites){const home=homes[sprite.index%homes.length];sprite.x=sprite.tx=home[0]+(sprite.role==='worker'?.6:0);sprite.y=sprite.ty=home[1]+(sprite.role==='worker'?.45:0);}
  if(config.judges||config.poc)sprites.push(...[{id:'CTO',role:'router',color:'#ffdf87',x:6,y:5.3,tx:5,ty:2,waiting:false,index:0,hit:null},{id:'Compliance Security & QA Engineer',role:'judge',color:'#a9b8ff',x:6,y:6.7,tx:10,ty:9,waiting:false,index:0,hit:null}]);
  if(config.company_mode)for(const [id,role,x,y,color,index] of [['Admin Judge','admin',9,9,'#b59bd8',6],['Operations Gent','tool',6,6,'#74a49c',7],['Training Gent','training',10.6,3.6,'#bd95c6',8],['CEO','oversight',2,2,'#d5a85e',9]])sprites.push({id,role,x,y,tx:x,ty:y,color,index,waiting:false});
  assignCrewLooks();
  $('agents').replaceChildren(...[...config.agents.map((a,i)=>[a.id,colors[i%colors.length]]),...(config.judges||config.poc?[["CTO",'#ffdf87'],["Compliance Security & QA Engineer",'#a9b8ff']]:[])].map(([id,color])=>{const d=document.createElement('button');d.type='button';d.className='badge';d.dataset.agent=id;d.style.borderColor=color;d.textContent=`${gentName(id)} · open dossier`;d.onclick=()=>showGent(id,true);return d;}));
  $('error').textContent='';await refreshHistory();updateJobBoard();const previous=localStorage.getItem(runKey());if(previous&&!working){try{const saved=await api(`/api/runs/${previous}`);await openRun(previous,saved);}catch(e){if(e.status===404)localStorage.removeItem(runKey());else throw e;}}await followLatestRun();await refreshAttention();updateSignal();
 }catch(e){if(e?.name==='AbortError')return;error(e);}
};
$('token').onchange=()=>{token=$('token').value.trim();};
$('logout').onclick=()=>{for(const request of pendingRequests)request.abort();finish();token='';$('token').value='';localStorage.removeItem(runKey());events=[];seen=0;runId=null;pending=null;$('history').replaceChildren();$('judgement').textContent='';for(const id of ['task','dod','required','forbidden'])$(id).value='';render();$('status').textContent='Signed out';};
$('login').onclick=async()=>{try{if(working)throw new Error('Finish or cancel the active view before switching sign-in');if(!window.__TAURI__)throw new Error('Use the macOS Tauri app for native sign-in; see the auth chapter.');token=await window.__TAURI__.core.invoke('login',{tenant:$('tenant').value.trim(),clientId:$('client').value.trim(),apiId:$('api').value.trim()});$('token').value='';$('status').textContent='Signed in · access token kept in memory';}catch(e){if(e?.name==='AbortError')return;error(e);}};
$('run').onclick=async()=>{
 if(working)return;
 try{
  if(!config)throw new Error('Connect first');
  working=true;$('run').disabled=true;$('cancel').disabled=true;$('error').textContent='';
  let criteria=lines('dod');
  const task=$('task').value.trim();
  if(config.company_mode&&criteria.length===0)criteria=['Deliver a useful, complete response to the requested task','State assumptions, evidence limits, and unresolved questions clearly'];
  let payload={task,consent:$('consent').checked,definition_of_done:criteria,acceptance_checks:{required_substrings:lines('required'),forbidden_substrings:lines('forbidden')}};
  const signature=JSON.stringify(payload);
  if(config.company_mode){
   if(!$('consent').checked)throw new Error('Review the request and check the authorization box before asking the Gents');
   if(!task)throw new Error('Describe the task you want the Gents to complete');
   if(new TextEncoder().encode(task).length>10000)throw new Error('Task must be at most 10,000 UTF-8 bytes');
   if(!pending||pending.signature!==signature){
    const scopedRun=crypto.randomUUID();
    const intake={idea:task,acceptance:criteria.join('\n'),deliverable_type:config.company_tools?'agent':'text',exclusions:'Unrequested paid services, credential disclosure, cross-company data access, destructive host actions without exact confirmation',timeline:'Continue across checkpoints until done or cancelled; 30-day session safety limit',company_signer:'Local operator browser approval',provider_signer:'Gents company',questions:'None supplied',budget_usd:0,signature_status:'unsigned'};
    const state=await api(`/api/companies/${encodeURIComponent(companyId)}`,{method:'POST',body:JSON.stringify({op:'intake',run_id:scopedRun,intake})});
    const job=state.jobs.find(item=>item.run_id===scopedRun);if(!job)throw new Error('The frozen company intake was not returned');
    await api(`/api/companies/${encodeURIComponent(companyId)}`,{method:'POST',body:JSON.stringify({op:'approve',run_id:scopedRun,digest:job.digest})});
    payload={...payload,company_id:companyId,run_id:scopedRun,consent:false,acceptance_checks:{required_substrings:[],forbidden_substrings:[]}};
    pending={signature,body:JSON.stringify(payload),key:crypto.randomUUID()};
   }
  }else{
   const body=JSON.stringify(payload);if(!pending||pending.body!==body)pending={body,key:crypto.randomUUID()};
  }
  const body=pending.body;
  const r=await api('/api/runs',{method:'POST',headers:{'Idempotency-Key':pending.key},body});
  pending=null;events=[];seen=0;runStarted=Date.now();$('judgement').textContent='';render();runId=r.id;
  localStorage.setItem(runKey(),runId);$('cancel').disabled=false;
  $('status').textContent='Run admitted';$('run-info').textContent=`Run ${runId} · starting`;updateSignal();poll=setTimeout(tick,100);refreshHistory().catch(error);
 }catch(e){if(e?.name==='AbortError')return;if([400,409,413].includes(e.status))pending=null;error(e);finish();}
};
async function refreshHistory(){
 const result=await api('/api/runs');
 runHistory=result.runs.filter(r=>!config?.company_mode||r.company_id===companyId);
 $('history').replaceChildren(...runHistory.map(r=>{const o=document.createElement('option');o.value=r.id;o.textContent=`${new Date(r.created_at_ms).toLocaleString()} · ${r.status}${r.task?` · ${r.task.slice(0,38)}`:''}`;return o;}));
 if(runId)$('history').value=runId;
 updateGentPanel();updateJobBoard();
}
async function openRun(id,saved=null){
 if(working)throw new Error('Cancel or finish the active view before opening another run');
 const r=saved||await api(`/api/runs/${id}`);if(config?.company_mode&&r.company_id!==companyId)throw new Error('Switch to the matching company before opening this run');runId=id;events=r.events;seen=0;applyEvents();render();localStorage.setItem(runKey(),id);
 $('status').textContent=`Run ${r.status}`;
 runStarted=r.events[0]?.timestamp_ms||Date.now();$('run-info').textContent=`Run ${runId} · ${r.status}`;if(r.status==='running'){working=true;$('run').disabled=true;$('cancel').disabled=false;poll=setTimeout(tick,100);}
}
$('resume').onclick=async()=>{try{const id=$('history').value;if(id){await openRun(id);followCompany=false;}}catch(e){if(e?.name==='AbortError')return;error(e);}};
$('delete-run').onclick=async()=>{try{const id=$('history').value;if(!id)return;await api(`/api/runs/${id}`,{method:'DELETE'});if(id===localStorage.getItem(runKey()))localStorage.removeItem(runKey());if(id===runId){events=[];seen=0;runId=null;render();}await refreshHistory();}catch(e){if(e?.name==='AbortError')return;error(e);}};
const esc=value=>String(value??'').replace(/[&<>'"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;',"'":'&#39;','"':'&quot;'}[c]));
const canonicalAgent=id=>id==='Routing Judge'||id==='cto'?'CTO':id==='Completion Judge'||id==='compliance-security-qa'?'Compliance Security & QA Engineer':id;
const orgRole=id=>config?.organization?.roles?.find(role=>role.id===positionKey(id));
const gentName=id=>{const canonical=canonicalAgent(id);if(canonical==='CTO')return config?.organization?.roles?.find(r=>r.id==='cto')?.title||'CTO';if(canonical==='Compliance Security & QA Engineer')return config?.organization?.roles?.find(r=>r.id==='compliance-security-qa')?.title||canonical;return orgRole(id)?.title||canonical;};
const sameAgent=(left,right)=>canonicalAgent(left)===canonicalAgent(right)||positionKey(left)===positionKey(right);
const friendlyKind=kind=>({workspace_ready:'Job opened',onboarding_started:'Onboarding',onboarding_completed:'Onboarding complete',route_accepted:'Team assigned',plan_accepted:'Planning together',working:'Building',testing:'Independent review',completion_review:'Completion review',sleep_started:'Dream cycle',dream_completed:'Memory cached',completed:'Job completed',blocked:'Job blocked',failed:'Job failed'}[kind]||kind?.replaceAll('_',' ')||'Available');
function currentRoute(){const event=[...events].reverse().find(e=>e.kind==='route_accepted');if(!event)return null;try{return JSON.parse(event.message);}catch{return null;}}
function usageFor(id){
 const calls=events.filter(e=>sameAgent(e.agent,id)&&e.reply),roles={planner:0,worker:0,router:0,judge:0,dreamer:0};let estimated=false;
 for(const e of calls){const u=e.reply.usage||{},known=(u.input??0)+(u.output??0);const tokens=known||Math.ceil((e.reply.text||'').length/4);if(!known)estimated=true;roles[e.role]=(roles[e.role]||0)+tokens;}
 const total=Object.values(roles).reduce((a,b)=>a+b,0);return {calls:calls.length,roles,total,estimated};
}
function gentProfile(id){
 if(id==='Training Gent'){const job=companyState?.training?.[0];return {id,name:`Training Gent · ${job?.manifest?.role||'role specialization'}`,specialty:'Offline LoRA training with reviewed examples, held-out evaluation and immutable base weights.',assignment:job?.manifest?.base_model||'Waiting for a dataset',latest:{kind:job?.progress?.phase||job?.state||'available'},usage:{total:0,calls:0,estimated:false},plannerPct:0,workerPct:0,plannerModel:'See Training Room metrics',workerModel:'MLX LoRA',publicEvents:[],route:null,isRouter:false,isJudge:false};}
 const agent=config?.agents?.find(a=>sameAgent(a.id,id)),role=orgRole(id),route=currentRoute(),assignment=route?.deployments?.find(x=>sameAgent(x.agent_id,id))||route?.assignments?.find(x=>sameAgent(x.agent,id));
 const latest=[...events].reverse().find(e=>sameAgent(e.agent,id)),usage=usageFor(id),workTotal=usage.roles.planner+usage.roles.worker;
 const plannerPct=workTotal?Math.round(usage.roles.planner/workTotal*100):0,workerPct=workTotal?100-plannerPct:0;
 const canonical=canonicalAgent(id),isRouter=canonical==='CTO',isJudge=canonical==='Compliance Security & QA Engineer';
 const specialty=role?.responsibility||(isRouter?'Owns staffing, routing, access, and production authorization':isJudge?'Independent compliance, security, accessibility, and quality evidence gate':agent?.specialty||agent?.assignment||(id.includes('Builder')?'Business software implementation':'Independent quality verification'));
 const configuredModel=config?.organization?.models?.find(m=>m.key===role?.model)?.display;
 const plannerModel=agent?.planner_model||configuredModel||config?.poc?.planner_model||config?.judges?.router_model||'Configured planner';
 const workerModel=agent?.worker_model||configuredModel||config?.poc?.worker_model||config?.judges?.completion_model||'Configured worker';
 const publicEvents=events.filter(e=>sameAgent(e.agent,id)&&['route_accepted','plan_accepted','agent_completed','completion_review','tool_result','dream_completed','onboarding_completed'].includes(e.kind)).slice(-12).reverse();
 return {id,name:role?.title||gentName(id),specialty,assignment:assignment?.responsibility||assignment?.assignment||agent?.specialty||agent?.assignment||(role?.scalable?'Available to be staffed by the CTO and PM':'Standing by'),latest,usage,plannerPct,workerPct,plannerModel,workerModel,publicEvents,route,isRouter,isJudge};
}
function updateJobBoard(){
 const latest=events.at(-1),current=runHistory.find(r=>r.id===runId);$('current-job').textContent=current?.task||$('task').value.trim()||'No active job';
 $('mobile-task-text').textContent=$('current-job').textContent;
 $('current-process').textContent=latest?`${gentName(latest.agent)} · ${friendlyKind(latest.kind)}`:(config?'Gents assembled · awaiting job':'Connect to assemble the Gents');
}
let activeGentPhase=null;
function showGent(id,pinned=false,phase=null){const changed=activeGent!==id||activeGentPhase!==phase;activeGent=id;activeGentPhase=phase;if(pinned){drawerPinned=true;clearHover();}$('gent-drawer').hidden=false;updateGentPanel();if(changed)$('gent-drawer').scrollTop=0;}
function hideGent(){if(drawerPinned)return;activeGent=null;$('gent-drawer').hidden=true;}
function updateGentPanel(){
 assignCrewLooks();
 if(config)decorateRoster();
 if(!activeGent||!config)return;const p=gentProfile(activeGent),history=runHistory.slice(0,20),assigned=p.route?(p.route.deployments||p.route.assignments||[]):[];
 const wardrobeOpen=$('gent-profile').querySelector('.gent-wardrobe')?.open,scrollTop=$('gent-drawer').scrollTop;
 const assignmentList=p.isRouter&&assigned.length?`<div class="dossier-section"><h4>First task assignments</h4><ul>${assigned.map(a=>`<li><strong>${esc(gentName(a.agent_id||a.agent))}</strong> — ${esc(a.responsibility||a.assignment||'Assigned')}</li>`).join('')}</ul></div>`:'';
 const rationale=p.publicEvents.length?p.publicEvents.map(e=>`<li><strong>${esc(friendlyKind(e.kind))}</strong> — ${esc(e.message).slice(0,500)}</li>`).join(''):'<li>No public decisions recorded yet.</li>';
 const jobs=history.length?history.map(r=>`<li>${esc(new Date(r.created_at_ms).toLocaleString())} · <strong>${esc(r.status)}</strong><br>${esc(r.task||'Legacy saved job')}</li>`).join(''):'<li>No previous jobs yet.</li>';
 $('gent-profile').innerHTML=`<span class="dossier-kicker">GENT DOSSIER / LIVE</span><h3>${esc(p.name)}</h3><p class="gent-specialty">${esc(p.specialty)}</p><span class="gent-state">${esc(friendlyKind(p.latest?.kind))}</span><div class="stat-grid"><div class="stat-card"><small>TOKENS</small><strong>${p.usage.total.toLocaleString()}${p.usage.estimated?' est.':''}</strong></div><div class="stat-card"><small>MODEL CALLS</small><strong>${p.usage.calls}</strong></div></div><div class="dossier-section"><h4>Current assignment</h4><p>${esc(p.assignment)}</p></div><div class="dossier-section"><h4>Planner / worker activity</h4><div class="role-meter"><div><span>Planner · ${p.plannerPct}%</span><span>${esc(p.plannerModel)}</span></div><span class="meter-track"><span class="meter-fill" style="width:${p.plannerPct}%"></span></span></div><div class="role-meter"><div><span>Worker · ${p.workerPct}%</span><span>${esc(p.workerModel)}</span></div><span class="meter-track"><span class="meter-fill" style="width:${p.workerPct}%"></span></span></div></div>${assignmentList}<div class="dossier-section"><h4>Public decisions & evidence</h4><ul>${rationale}</ul><p class="privacy-note">Observable rationale, plans, actions, and evidence are shown. Private chain-of-thought is not recorded or exposed.</p></div><div class="dossier-section"><h4>Current & previous jobs</h4><ul>${jobs}</ul></div>`;
 const model=modelForGent(activeGent,activeGentPhase),character=characterFor(activeGent,activeGentPhase),key=character.appearanceKey;
 $('gent-profile').insertAdjacentHTML('afterbegin',`<figure class="gent-portrait" data-position="${esc(key)}" data-model="${esc(modelKey(model))}" data-character="${character.id}" style="--gent-accent:${character.accent}"><img src="${character.src}" alt="${esc(character.name)} in a suit — ${esc(gentName(activeGent))}"><figcaption><strong>${esc(character.name)}</strong><span>${esc(activeGentPhase||'Position')} · ${esc(model)}</span></figcaption></figure>`);
 const wardrobe=document.createElement('details');wardrobe.className='gent-wardrobe';
 const summary=document.createElement('summary');summary.textContent=`Choose this position’s character${activeGentPhase?` · ${activeGentPhase}`:''}`;wardrobe.append(summary);
 const choices=document.createElement('div');choices.className='character-choices';choices.setAttribute('aria-label','Position character appearances');
 for(const look of characters){const button=document.createElement('button');button.type='button';button.className='character-choice';const owner=[...crewLooks].find(([ownerKey,value])=>value?.id===look.id&&ownerKey!==key);button.disabled=!!owner;button.title=owner?`${look.name} is already assigned to ${owner[0]}`:look.name;button.setAttribute('aria-label',look.name);button.setAttribute('aria-pressed',String(look.id===character.id));const img=document.createElement('img');img.src=look.src;img.alt='';button.append(img);button.onclick=()=>{avatarChoices[key]=look.id;try{localStorage.setItem('gents:unique-crew:v2',JSON.stringify(avatarChoices));}catch{}updateGentPanel();$('gent-profile').querySelector('.gent-wardrobe').open=true;};choices.append(button);}
 const note=document.createElement('p');note.className='wardrobe-note';note.textContent='One identity per Gent. Assigned faces are unavailable to prevent duplicates.';wardrobe.append(note);
 wardrobe.open=!!wardrobeOpen;wardrobe.append(choices);$('gent-profile').querySelector('.gent-portrait').after(wardrobe);$('gent-drawer').scrollTop=scrollTop;
}
function modelForGent(id,phase=null){
 const latest=[...events].reverse().find(e=>sameAgent(e.agent,id)&&e.model&&(!phase||e.role===phase||phase==='worker'&&e.role==='judge'));
 if(latest)return latest.model.replace(/^(local|mlx|mock)\s*\/\s*/i,'');
 if(id==='Admin Judge')return config?.admin_judge||'Qwen3-8B';
 if(id==='Operations Gent')return 'Operations coordinator';
 if(id==='Training Gent')return companyState?.training?.[0]?.manifest?.base_model||'Training worker';
 const agent=config?.agents?.find(a=>sameAgent(a.id,id));
 return agent?.[`${phase||'worker'}_model`]||config?.organization?.models?.find(m=>m.key===orgRole(id)?.model)?.display||(id==='CEO'?'Qwen3-8B':'Unassigned');
}
function decorateRoster(){
 for(const button of $('org-chart').querySelectorAll('.org-card')){
  const role=config?.organization?.roles?.find(r=>r.title===button.querySelector('strong')?.textContent),look=characterFor(role?.id);
  if(button.dataset.character===look.id)continue;
  button.querySelector('.roster-portrait')?.remove();const img=document.createElement('img');img.className='roster-portrait';img.src=look.src;img.alt='';img.loading='lazy';img.decoding='async';button.prepend(img);button.dataset.character=look.id;button.dataset.position=look.position;button.style.setProperty('--gent-accent',look.accent);
 }
}
$('close-gent').onclick=()=>{drawerPinned=false;activeGent=null;activeGentPhase=null;clearHover();$('gent-drawer').hidden=true;};
function applyEvents(){for(const e of events.slice(seen)){if(e.kind==='round_started')$('judgement').textContent=e.message;if(e.kind==='completion_review'){const v=JSON.parse(e.message);$('judgement').textContent=`Compliance Security & QA: ${v.verdict} · ${v.summary}`;}if(e.kind==='workspace_ready'){try{$('artifact').textContent=`Artifact: ${JSON.parse(e.message).artifact}`;}catch{}}if(e.kind==='route_accepted'){const route=JSON.parse(e.message);const assigned=route.deployments||route.assignments||[];document.querySelectorAll('.badge').forEach(b=>{const d=assigned.find(x=>sameAgent(x.agent_id||x.agent,b.dataset.agent));b.textContent=d?`${gentName(d.agent_id||d.agent)} · assigned`:`${gentName(b.dataset.agent)} · open dossier`;b.style.opacity=d?'1':'.68';});for(const d of assigned){for(const sprite of sprites.filter(s=>sameAgent(s.id,d.agent_id||d.agent))){sprite.tx=3+sprite.index*.45+(sprite.role==='worker'?.25:-.25);sprite.ty=3+(sprite.role==='worker'?.38:-.3);}}}const together=['planner','onboarding'].includes(e.role),targets=e.role==='dreamer'||together?sprites.filter(s=>sameAgent(s.id,e.agent)):sprites.filter(s=>sameAgent(s.id,e.agent)&&s.role===e.role);for(const sprite of targets){const [x,y]=rooms[e.room]||rooms.done;sprite.tx=x+sprite.index*.38+(sprite.role==='worker'?.25:-.25);sprite.ty=y+(sprite.role==='worker'?.42:-.32);sprite.waiting=['call_started','loading','sleep_started'].includes(e.kind);}}seen=events.length;updateJobBoard();updateGentPanel();}
async function tick(){try{const [r,trace]=await Promise.all([api(`/api/runs/${runId}`),api(`/api/runs/${runId}/observability`).catch(()=>observability)]);backendOnline=true;$('error').textContent='';events=r.events;observability=trace;applyEvents();render();renderObservability();await refreshAttention();const elapsed=Math.max(0,Math.round((Date.now()-runStarted)/1000)),latest=events.at(-1);$('status').textContent=`Run ${r.status}${latest?` · ${latest.kind}`:''}`;$('run-info').textContent=`Run ${runId} · ${r.status} · ${elapsed}s${latest?.model?` · ${latest.agent}/${latest.role} · ${latest.model}`:''}`;if(r.status==='running')poll=setTimeout(tick,1500);else{finish();await refreshHistory();}updateSignal();}catch(e){if(e?.name==='AbortError')return;backendOnline=false;error(e);updateSignal();poll=setTimeout(tick,3000);}}
function finish(){working=false;clearTimeout(poll);$('run').disabled=false;$('cancel').disabled=true;}
$('cancel').onclick=async()=>{try{await api(`/api/runs/${runId}/cancel`,{method:'POST'});$('status').textContent=config?.poc?'Cancel requested · owned local work will stop':'Cancel requested · active call may finish and be billed';}catch(e){if(e?.name==='AbortError')return;error(e);}};
const n=x=>x===null||x===undefined?'unknown':x;
function render(){
 updateSignal();updateSpriteActivity();
 $('break-room-log').textContent=events.slice(-100).map(e=>`${new Date(e.timestamp_ms).toLocaleTimeString()}  ${e.agent} / ${e.role}\n${e.model||''}\n${e.message}${e.reply?'\n'+e.reply.text:''}`).join('\n\n');
 const calls=events.filter(e=>e.reply);const completed=new Set(calls.map(e=>e.call_id).filter(Boolean));const unresolved=events.filter(e=>e.kind==='call_started'&&e.call_id&&!completed.has(e.call_id)).length;const sum=k=>{const known=calls.filter(e=>e.reply.usage[k]!=null);return `${known.reduce((s,e)=>s+e.reply.usage[k],0).toLocaleString(undefined,{maximumFractionDigits:k==='cost_usd'?8:0})}${known.length<calls.length?' + unknown':''}`;};
 $('totals').textContent=`${calls.length} completed calls · input: ${sum('input')} · output: ${sum('output')} · provider-reported USD: ${sum('cost_usd')} · ${unresolved} failed/unfinished calls may have unknown charges`;
 $('events').replaceChildren(...events.slice(-80).reverse().map(e=>{const d=document.createElement('details');d.className='event';const title=document.createElement('summary');title.textContent=`#${e.seq} ${new Date(e.timestamp_ms).toLocaleTimeString()} ${e.agent} / ${e.role} · ${e.kind}`;d.append(title);const p=document.createElement('pre');p.textContent=(e.call_id?`Call ${e.call_id}\n`:'')+(e.model?e.model+'\n':'')+e.message+(e.reply?'\n\n'+e.reply.text:'');d.append(p);if(e.reply){const m=document.createElement('div');m.className='metrics';const u=e.reply.usage;m.textContent=`Input ${n(u.input)} · output ${n(u.output)} · cached subset ${n(u.cached_input)} · reasoning subset ${n(u.reasoning)} · USD ${n(u.cost_usd)} · ${e.reply.latency_ms} ms · ${u.source}`;d.append(m);}return d;}));
}
async function refreshCompany(){
 companyState=await api(`/api/companies/${encodeURIComponent(companyId)}`,{method:'POST',body:JSON.stringify({op:'status'})});
 $('company-name').textContent=companyState.name;
 $('company-context').textContent=companyState.tiers.map(t=>`${t.tier}: ${t.records}`).join(' · ')||'No context records yet';
 renderLearning();
 $('capability-status').textContent=config?.company_tools?`Admin judge: ${config.admin_judge||'configured local model'} · ${Object.keys(config.tools||{}).length} tools available`:'Company observation mode';
}
let lastMeeting=null,companyPollBusy=false;
function renderLearning(){
 $('learning-operations').hidden=!config?.company_mode;
 const jobs=companyState?.training||[];
 $('training-status').innerHTML=jobs.length?jobs.slice(0,4).map(job=>{const m=job.progress?.metrics||{},phase=['candidate','active','retired','failed','cancelled'].includes(job.state)?job.state:(job.progress?.phase||job.state);return `<article class="training-job"><strong>${esc(job.manifest.role)} · ${esc(phase)}</strong><span>${esc(job.manifest.base_model)}</span><small>${esc(job.id)}</small><p>${job.manifest.smoke?'Synthetic smoke test · promotion disabled':`Reviewed role dataset · ${esc(job.manifest.iterations)} iterations`}</p>${m.baseline_test_loss!==undefined?`<p>Held-out loss ${Number(m.baseline_test_loss).toFixed(4)} → ${Number(m.adapter_test_loss).toFixed(4)} · ${Number(m.peak_memory_gb).toFixed(1)} GB peak</p>`:m.iteration!==undefined?`<p>Iteration ${esc(m.iteration)} · training loss ${esc(m.train_loss)}</p>`:''}<small>Training and inference share one GPU queue.</small></article>`;}).join(''):'<p>No training jobs. Prepare reviewed examples through the CLI.</p>';
 const running=companyState?.jobs?.filter(j=>j.state==='running')||[],overdue=(companyState?.milestones||[]).filter(m=>m.state!=='done'&&m.due_ms&&m.due_ms<Date.now());
 $('ceo-status').textContent=`CEO observes active work every 30 seconds; model reviews follow progress or risk. ${running.length} active jobs · ${overdue.length} overdue milestones. Unknown deadlines remain unknown.`;
 const meeting=companyState?.meetings?.[0];$('meeting-brief').textContent=meeting?.brief||'No all-hands meeting called.';
 if(!sprites.some(s=>s.id==='CEO'))sprites.push({id:'CEO',role:'oversight',color:'#f0ca75',x:2,y:2,tx:2,ty:2,index:0,waiting:false,roamAt:Infinity});
 if(jobs.length&&!sprites.some(s=>s.id==='Training Gent'))sprites.push({id:'Training Gent',role:'training',color:'#c4b2ed',x:10.3,y:3.1,tx:10.3,ty:3.1,index:1,waiting:false,roamAt:Infinity});
 const trainee=sprites.find(s=>s.id==='Training Gent');if(trainee){trainee.waiting=jobs.some(j=>j.state==='queued'&&j.progress?.phase&&j.progress.phase!=='candidate');trainee.tx=10.3;trainee.ty=3.1;}
 if(meeting&&meeting.id!==lastMeeting){lastMeeting=meeting.id;conferenceUntil=Date.now()+15000;for(const [i,s] of sprites.filter(s=>s.role!=='training').entries()){s.tx=1.7+(i%5)*.55;s.ty=8.5+Math.floor(i/5)*.7;s.roamAt=performance.now()+15000;}}
}
async function followLatestRun(){
 if(!config?.company_mode||!followCompany||working)return;
 const latest=runHistory.find(r=>r.status==='running')||runHistory[0];
 if(latest&&latest.id!==runId)await openRun(latest.id);
}
async function syncCompanyView(){
 if(!config?.company_mode||companyPollBusy)return;companyPollBusy=true;
 try{await refreshCompany();await refreshHistory();await followLatestRun();await refreshAttention();backendOnline=true;updateSignal();}
 catch{backendOnline=false;updateSignal();}finally{companyPollBusy=false;}
}
setInterval(syncCompanyView,5000);
document.addEventListener('visibilitychange',()=>{if(!document.hidden){syncCompanyView();if(working){clearTimeout(poll);tick();}}});
window.addEventListener('online',()=>{syncCompanyView();if(working){clearTimeout(poll);tick();}});
$('watch-company').onclick=async()=>{try{if(working)throw new Error('Finish the current view before changing company');followCompany=true;companyId=$('company-id').value.trim();events=[];seen=0;runId=null;activeGent=null;$('gent-drawer').hidden=true;render();await refreshCompany();await refreshHistory();await followLatestRun();}catch(e){error(e);}};
$('export').onclick=()=>{const url=URL.createObjectURL(new Blob([JSON.stringify({runId,events},null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='gents-events.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
$('focus-request').onclick=()=>{$('request-desk').scrollIntoView({behavior:'smooth',block:'start'});$('task').focus();};
document.querySelectorAll('[data-mobile-target]').forEach(button=>button.onclick=()=>$(button.dataset.mobileTarget).scrollIntoView({behavior:'smooth',block:'start'}));
$('office-zoom').oninput=()=>{clearHover();canvas.style.width=`${$('office-zoom').value}%`;$('office-zoom-value').textContent=`${$('office-zoom').value}%`;};
$('mobile-disconnect').onclick=async()=>{try{const response=await fetch('/mobile/session',{method:'DELETE'});if(response.ok)location.reload();else throw new Error('Could not disconnect this phone');}catch(e){error(e);}};
async function mobileStatus(){
 if(['localhost','127.0.0.1','[::1]'].includes(location.hostname))return;
 try{const response=await fetch('/mobile/status');if(!response.ok)return;const status=await response.json();if(status.execution==='laptop'){mobileAccessMode=true;$('mobile-access').hidden=false;$('base').readOnly=true;$('mobile-computer').textContent=`Running on ${status.computer}. This phone is a control panel; closing it does not stop work.`;}}catch{}
}
$('restart-company').onclick=async()=>{try{await api('/api/company/restart',{method:'POST',body:'{}'});backendOnline=false;updateSignal();}catch(e){$('attention-error').textContent=e.message;}};
async function refreshAttention(){
 if(!config?.company_tools)return;
 const ids=[...new Set([working?runId:null,...(companyState?.jobs||[]).filter(j=>j.state==='running').map(j=>j.run_id)].filter(Boolean))];
 attention=null;attentionRun=null;
 for(const id of ids){try{const state=await api(`/api/runs/${id}/attention`);if(state.attention&&!state.attention.answer){attention=state.attention;attentionRun=id;break;}}catch(e){if(e.status!==404)throw e;}}
 $('attention-panel').hidden=!attention;
 if(attention)$('attention-question').textContent=attention.question;
}
$('attention-form').onsubmit=async e=>{
 e.preventDefault();if(!attentionRun||!attention)return;
 const button=e.currentTarget.querySelector('button');button.disabled=true;
 try{await api(`/api/runs/${attentionRun}/attention`,{method:'POST',body:JSON.stringify({question_id:attention.id,answer:$('attention-answer').value})});$('attention-answer').value='';$('attention-error').textContent='';attention=null;await refreshAttention();updateSignal();}
 catch(e){$('attention-error').textContent=e.message;}finally{button.disabled=false;}
};
function updateSignal(){
 const active=(companyState?.jobs||[]).some(j=>j.state==='running')||working;
 const training=(companyState?.training||[]).some(j=>['running','queued'].includes(j.state));
 const state=attention||config&&!backendOnline?'phone':active||training?'green':'red';
 $('company-signal').dataset.state=state;
 $('signal-title').textContent=state==='phone'?'CODEX HOTLINE':state==='green'?'COMPANY AT WORK':'OFF DUTY';
 $('signal-detail').textContent=attention?'Red phone ringing · advisory escalation':config&&!backendOnline?'Reconnecting to the office…':state==='green'?'The Gents are on the case':'Ready for the next mission';
 codexPhone.dataset.ringing=String(!!attention||events.slice(-12).some(e=>['local_executive_ringing','codex_phone_ringing'].includes(e.kind)&&!events.slice(-12).some(done=>done.seq>e.seq&&['question_resolved_locally','question_resolved_by_openai','codex_phone_unanswered','cloud_escalation_failed'].includes(done.kind))));
 const cards=[['THE MISSION',attention?'Your answer unlocks the next step':active?'Work in progress':'Your next big idea goes here',active?'active':'rest'],['ADMIN DESK',config?.admin_judge||'Local judgment',attention?'waiting':'rest'],['TRAINING & OPERATIONS',training?'Local training in progress':'One model · one shared GPU',training?'active':'rest']];
 $('job-cards').replaceChildren(...cards.map(([title,text,state])=>{const card=document.createElement('article');card.className='job-card';card.dataset.state=state;const label=document.createElement('small');label.textContent=title;const content=document.createElement('strong');content.textContent=text;const rail=document.createElement('span');rail.className='job-rail';card.append(label,content,rail);return card;}));
}
function updateSpriteActivity(){
 const resolved=new Set(events.filter(e=>e.reply).map(e=>e.call_id));
 for(const sprite of sprites){
  const matching=events.filter(e=>sameAgent(e.agent,sprite.id)&&(e.role===sprite.role||sprite.role==='worker'&&e.role==='judge'||['Operations Gent','Training Gent','CEO'].includes(sprite.id)));
  const latest=matching.at(-1);
  sprite.busy=backendOnline&&working&&!attention&&!!latest&&(['loading','tool_started','context_retrieved'].includes(latest.kind)||latest.kind==='call_started'&&!resolved.has(latest.call_id));
  sprite.activity=latest?.kind==='loading'?'Loading local model':latest?.kind==='context_retrieved'?'Reading handoff context':latest?.kind==='call_started'?(sprite.role==='admin'?'Reviewing the next action':sprite.role==='planner'?'Planning the next step':'Working on the mission'):latest?.kind==='tool_started'?(()=>{try{return JSON.parse(latest.message).action.replaceAll('_',' ');}catch{return 'Running a tool';}})():'';
  if(sprite.busy&&latest){const home=rooms[latest.room]||rooms.operations;sprite.tx=home[0]+(sprite.role==='planner'?-.35:.25);sprite.ty=home[1]+(sprite.index%3)*.32;}
  if(sprite.id==='Training Gent'&&backendOnline){sprite.busy=(companyState?.training||[]).some(j=>j.state==='running');sprite.activity=sprite.busy?'Training local adapter':'';}
 }
}
const hoverPreview=document.createElement('div');hoverPreview.id='gent-hover-preview';hoverPreview.hidden=true;hoverPreview.setAttribute('role','tooltip');canvas.parentElement.append(hoverPreview);
let hoverTimer=null,leaveTimer=null,hoverSprite=null,pointerDown=null;
function clearHover(){clearTimeout(hoverTimer);clearTimeout(leaveTimer);hoverTimer=null;leaveTimer=null;hoverGent=null;hoverFloor=null;hoverSprite=null;hoverPreview.hidden=true;canvas.style.cursor='default';}
function positionHover(){
 if(hoverPreview.hidden||!hoverSprite?.hit)return;
 const stage=canvas.parentElement.getBoundingClientRect(),r=canvas.getBoundingClientRect(),h=hoverSprite.hit;
 const anchor={left:r.left-stage.left+h.x*r.width/canvas.width,right:r.left-stage.left+(h.x+h.w)*r.width/canvas.width,top:r.top-stage.top+h.y*r.height/canvas.height};
 const pos=previewPosition(anchor,stage,{width:hoverPreview.offsetWidth,height:hoverPreview.offsetHeight});hoverPreview.style.left=`${pos.left}px`;hoverPreview.style.top=`${pos.top}px`;
}
function showHover(s){
 if(drawerPinned)return;const model=modelForGent(s.id,s.role),look=characterFor(s.id,s.role);hoverSprite=s;hoverPreview.dataset.position=look.appearanceKey;hoverPreview.dataset.character=look.id;hoverPreview.style.setProperty('--gent-accent',look.accent);
 hoverPreview.innerHTML=`<img src="${look.src}" alt="${esc(look.name)} original portrait"><div><strong>${esc(gentName(s.id))}</strong><span>${esc(look.name)} · ${esc(s.role)}</span><span>${esc(model)}</span><p>${esc(s.busy?s.activity||'Working':'Resting · ZZZZ')}</p><small>Click for dossier & character choices</small></div>`;
 hoverPreview.hidden=false;positionHover();
}
function gentAt(e){
 const r=canvas.getBoundingClientRect(),x=(e.clientX-r.left)*canvas.width/r.width,y=(e.clientY-r.top)*canvas.height/r.height;
 return [...sprites].sort((a,b)=>b.paintOrder-a.paintOrder).find(s=>spriteContains(s.hit,x,y));
}
function floorAt(e){
 if(towerFilter!=='all')return null;const r=canvas.getBoundingClientRect(),x=(e.clientX-r.left)*canvas.width/r.width,y=(e.clientY-r.top)*canvas.height/r.height;
 return canvas.floorHits?.find(f=>x>=f.x&&x<=f.x+f.w&&y>=f.y&&y<=f.y+f.h);
}
canvas.addEventListener('pointermove',e=>{
 if(e.pointerType==='touch'||drawerPinned)return;const gent=gentAt(e),floor=floorAt(e),key=gent?`${gent.id}:${gent.role}`:null;hoverFloor=floor?.id||null;canvas.style.cursor=gent||floor?'pointer':'default';
 if(gent){clearTimeout(leaveTimer);leaveTimer=null;if(key===hoverGent&&!hoverPreview.hidden)return;clearTimeout(hoverTimer);hoverGent=key;hoverPreview.hidden=true;hoverTimer=setTimeout(()=>showHover(gent),180);}
 else{clearTimeout(hoverTimer);if(hoverGent&&!leaveTimer)leaveTimer=setTimeout(()=>{clearHover();leaveTimer=null;},160);}
});
canvas.addEventListener('pointerleave',()=>{clearTimeout(hoverTimer);clearTimeout(leaveTimer);leaveTimer=setTimeout(clearHover,160);});
canvas.addEventListener('pointerdown',e=>{pointerDown={x:e.clientX,y:e.clientY};});
canvas.addEventListener('pointerup',e=>{if(!pointerDown||Math.hypot(e.clientX-pointerDown.x,e.clientY-pointerDown.y)>8)return;pointerDown=null;const floor=floorAt(e);if(floor){selectTowerFloor(String(floor.id));return;}const gent=gentAt(e);if(gent)showGent(gent.id,true,gent.role);});
canvas.addEventListener('pointercancel',()=>{pointerDown=null;clearHover();});
document.addEventListener('keydown',e=>{if(e.key==='Escape')$('close-gent').click();});
window.addEventListener('resize',clearHover);window.addEventListener('scroll',clearHover,{passive:true});
let lastTowerFrame=0;
function frame(now=performance.now()){
 if(now-lastTowerFrame<33){requestAnimationFrame(frame);return;}lastTowerFrame=now;
 const phoneActive=codexPhone.dataset.ringing==='true',conferenceActive=Date.now()<conferenceUntil;
 const stats=renderTower(ctx,{crew:sprites,characterFor:(id,phase)=>crewLooks.get(appearanceKey(id,phase)),spriteFrames,filter:towerFilter,hoverGent,hoverFloor,time:now,preview:walkingPreview,reducedMotion:motionPreference.matches,conferenceActive,phoneActive});
 const summary=`${stats.active} working · ${stats.resting} in recharge · ${sprites.length-stats.unassigned} unique Gents${stats.unassigned?' · More character art required':''}`;
 if(towerSummary.textContent!==summary)towerSummary.textContent=summary;
 canvas.dataset.active=String(stats.active);canvas.dataset.resting=String(stats.resting);
 updateCrewManifest();
 positionHover();
 requestAnimationFrame(frame);
}
frame();
mobileStatus();
if(new URLSearchParams(location.search).get('autoconnect')==='1'||!['localhost','127.0.0.1','[::1]'].includes(location.hostname)&&location.protocol==='https:')$('connect').click();
