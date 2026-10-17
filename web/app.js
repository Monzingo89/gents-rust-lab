const $=id=>document.getElementById(id);
let base=location.protocol.startsWith('http')&&location.port==='8787'?location.origin:'http://127.0.0.1:8787';
$('base').value=base;
let config=null,runId=null,events=[],seen=0,poll=null,token='',sprites=[],working=false,pending=null;
const lines=id=>$(id).value.split('\n').map(x=>x.trim()).filter(Boolean);
const runKey=()=>`gents-run:${base}`;
const rooms={planning:[215,165],library:[665,165],workshop:[215,440],review:[665,440],done:[450,305],routing:[365,125],judging:[805,420]};
const colors=['#9de3bf','#e4b4ff','#ffcf8d','#8ed6ff','#f89dbe','#e7e794','#a2a3ff','#77d8d4'];
const ctx=$('office').getContext('2d');
function error(e){if(e?.name==='AbortError')return;$('error').textContent=e.message||String(e);}
const pendingRequests=new Set();
async function api(path,options={}){
 const controller=new AbortController();pendingRequests.add(controller);
 try{
  const headers={'Content-Type':'application/json',...options.headers};if(token)headers.Authorization=`Bearer ${token}`;
  const r=await fetch(base+path,{...options,headers,signal:controller.signal});
  if(!r.ok){const e=new Error(`${r.status}: ${await r.text()}`);e.status=r.status;throw e;}
  return await r.json();
 }finally{pendingRequests.delete(controller);}
}
$('connect').onclick=async()=>{try{if(working)throw new Error('Finish or cancel the active view before changing connections');const u=new URL($('base').value);if(u.protocol!=='https:'&&!(u.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(u.hostname)))throw new Error('Use HTTPS for a remote harness.');base=u.origin;token=$('token').value.trim();config=await api('/api/config');$('status').textContent=`Connected · ${config.auth} · ${config.agents.length} agents`;$('providers').textContent=[...new Set(config.agents.flatMap(a=>[a.planner_provider,a.worker_provider]).concat(config.judges?[config.judges.router_provider,config.judges.completion_provider]:[]).concat(config.catalog_providers||[]))].join(', ');sprites=config.agents.flatMap((a,i)=>['planner','worker'].map((role,j)=>({id:a.id,role,color:colors[i%colors.length],x:390+i*12,y:290+j*25,tx:390+i*12,ty:290+j*25,waiting:false,index:i})));if(config.judges){sprites.push(...[{id:'Routing Judge',role:'router',color:'#ffdf87',x:450,y:250,tx:365,ty:125,waiting:false,index:0},{id:'Completion Judge',role:'judge',color:'#a9b8ff',x:450,y:350,tx:805,ty:420,waiting:false,index:0}]);} $('agents').replaceChildren(...config.agents.map((a,i)=>{const d=document.createElement('div');d.className='badge';d.dataset.agent=a.id;d.style.borderColor=colors[i%colors.length];d.textContent=config.judges?`${a.id} · available; routing judge selects models per round`:`${a.id} · planner: ${a.planner_provider} · worker: ${a.worker_provider}`;return d;}));$('error').textContent='';await refreshHistory();const previous=localStorage.getItem(runKey());if(previous&&!working){const saved=await api(`/api/runs/${previous}`);await openRun(previous,saved);}}catch(e){if(e?.name==='AbortError')return;error(e);}};
$('token').onchange=()=>{token=$('token').value.trim();};
$('logout').onclick=()=>{for(const request of pendingRequests)request.abort();finish();token='';$('token').value='';localStorage.removeItem(runKey());events=[];seen=0;runId=null;pending=null;$('history').replaceChildren();$('judgement').textContent='';for(const id of ['task','dod','required','forbidden'])$(id).value='';render();$('status').textContent='Signed out';};
$('login').onclick=async()=>{try{if(working)throw new Error('Finish or cancel the active view before switching sign-in');if(!window.__TAURI__)throw new Error('Use the macOS Tauri app for native sign-in; see the auth chapter.');token=await window.__TAURI__.core.invoke('login',{tenant:$('tenant').value.trim(),clientId:$('client').value.trim(),apiId:$('api').value.trim()});$('token').value='';$('status').textContent='Signed in · access token kept in memory';}catch(e){if(e?.name==='AbortError')return;error(e);}};
$('run').onclick=async()=>{
 if(working)return;
 try{
  if(!config)throw new Error('Connect first');
  working=true;$('run').disabled=true;$('cancel').disabled=true;$('error').textContent='';
  const payload={task:$('task').value,consent:$('consent').checked,definition_of_done:lines('dod'),acceptance_checks:{required_substrings:lines('required'),forbidden_substrings:lines('forbidden')}};
  const body=JSON.stringify(payload);
  if(!pending||pending.body!==body)pending={body,key:crypto.randomUUID()};
  const r=await api('/api/runs',{method:'POST',headers:{'Idempotency-Key':pending.key},body});
  pending=null;events=[];seen=0;render();runId=r.id;
  localStorage.setItem(runKey(),runId);$('cancel').disabled=false;
  $('status').textContent='Run admitted';poll=setTimeout(tick,100);refreshHistory().catch(error);
 }catch(e){if(e?.name==='AbortError')return;if([400,409,413].includes(e.status))pending=null;error(e);finish();}
};
async function refreshHistory(){
 const result=await api('/api/runs');
 $('history').replaceChildren(...result.runs.map(r=>{const o=document.createElement('option');o.value=r.id;o.textContent=`${new Date(r.created_at_ms).toLocaleString()} · ${r.status}`;return o;}));
 if(runId)$('history').value=runId;
}
async function openRun(id,saved=null){
 if(working)throw new Error('Cancel or finish the active view before opening another run');
 const r=saved||await api(`/api/runs/${id}`);runId=id;events=r.events;seen=0;applyEvents();render();localStorage.setItem(runKey(),id);
 $('status').textContent=`Run ${r.status}`;
 if(r.status==='running'){working=true;$('run').disabled=true;$('cancel').disabled=false;poll=setTimeout(tick,100);}
}
$('resume').onclick=async()=>{try{const id=$('history').value;if(id)await openRun(id);}catch(e){if(e?.name==='AbortError')return;error(e);}};
$('delete-run').onclick=async()=>{try{const id=$('history').value;if(!id)return;await api(`/api/runs/${id}`,{method:'DELETE'});if(id===localStorage.getItem(runKey()))localStorage.removeItem(runKey());if(id===runId){events=[];seen=0;runId=null;render();}await refreshHistory();}catch(e){if(e?.name==='AbortError')return;error(e);}};
function applyEvents(){for(const e of events.slice(seen)){if(e.kind==='round_started')$('judgement').textContent=e.message;if(e.kind==='completion_review'){const v=JSON.parse(e.message);$('judgement').textContent=`Completion judge: ${v.verdict} · ${v.summary}`;}if(e.kind==='route_accepted'){const route=JSON.parse(e.message);document.querySelectorAll('.badge').forEach(b=>{const d=route.deployments.find(x=>x.agent_id===b.dataset.agent);b.textContent=d?`${d.agent_id} · planner: ${d.planner_model} · worker: ${d.worker_model} · context: ${d.context_from.join(', ')||'task only'}`:`${b.dataset.agent} · not deployed this round`;b.style.opacity=d?'1':'.45';});}const sprite=sprites.find(s=>s.id===e.agent&&s.role===e.role);if(sprite){const [x,y]=rooms[e.room]||rooms.done;sprite.tx=x+sprite.index*18;sprite.ty=y+(e.role==='worker'?30:-20);sprite.waiting=e.kind==='call_started';}}seen=events.length;}
async function tick(){try{const r=await api(`/api/runs/${runId}`);events=r.events;applyEvents();render();$('status').textContent=`Run ${r.status}`;if(r.status==='running')poll=setTimeout(tick,700);else{finish();await refreshHistory();}}catch(e){if(e?.name==='AbortError')return;error(e);finish();}}
function finish(){working=false;clearTimeout(poll);$('run').disabled=false;$('cancel').disabled=true;}
$('cancel').onclick=async()=>{try{await api(`/api/runs/${runId}/cancel`,{method:'POST'});$('status').textContent='Cancel requested · active call may finish and be billed';}catch(e){if(e?.name==='AbortError')return;error(e);}};
const n=x=>x===null||x===undefined?'unknown':x;
function render(){
 const calls=events.filter(e=>e.reply);const completed=new Set(calls.map(e=>e.call_id).filter(Boolean));const unresolved=events.filter(e=>e.kind==='call_started'&&e.call_id&&!completed.has(e.call_id)).length;const sum=k=>{const known=calls.filter(e=>e.reply.usage[k]!=null);return `${known.reduce((s,e)=>s+e.reply.usage[k],0).toLocaleString(undefined,{maximumFractionDigits:k==='cost_usd'?8:0})}${known.length<calls.length?' + unknown':''}`;};
 $('totals').textContent=`${calls.length} completed calls · input: ${sum('input')} · output: ${sum('output')} · provider-reported USD: ${sum('cost_usd')} · ${unresolved} failed/unfinished calls may have unknown charges`;
 $('events').replaceChildren(...[...events].reverse().map(e=>{const d=document.createElement('details');d.className='event';const title=document.createElement('summary');title.textContent=`#${e.seq} ${new Date(e.timestamp_ms).toLocaleTimeString()} ${e.agent} / ${e.role} · ${e.kind}`;d.append(title);const p=document.createElement('pre');p.textContent=(e.call_id?`Call ${e.call_id}\n`:'')+(e.model?e.model+'\n':'')+e.message+(e.reply?'\n\n'+e.reply.text:'');d.append(p);if(e.reply){const m=document.createElement('div');m.className='metrics';const u=e.reply.usage;m.textContent=`Input ${n(u.input)} · output ${n(u.output)} · cached subset ${n(u.cached_input)} · reasoning subset ${n(u.reasoning)} · USD ${n(u.cost_usd)} · ${e.reply.latency_ms} ms · ${u.source}`;d.append(m);}return d;}));
}
$('export').onclick=()=>{const url=URL.createObjectURL(new Blob([JSON.stringify({runId,events},null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='gents-events.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
function frame(){
 ctx.fillStyle='#0b1321';ctx.fillRect(0,0,900,600);
 for(const [name,x,y,w,h] of [['PLANNING',18,18,415,245],['LIBRARY / HANDOFF',467,18,415,245],['WORKSHOP',18,337,415,245],['REVIEW',467,337,415,245]]){ctx.fillStyle='#1b2a40';ctx.fillRect(x,y,w,h);ctx.strokeStyle='#3f5773';ctx.lineWidth=4;ctx.strokeRect(x,y,w,h);ctx.fillStyle='#aac1df';ctx.font='14px monospace';ctx.fillText(name,x+18,y+30);ctx.fillStyle='#344b65';ctx.fillRect(x+65,y+85,120,40);ctx.fillRect(x+235,y+140,110,40);}
 ctx.fillStyle='#a2b6cc';ctx.font='12px monospace';ctx.fillText('HARNESS CORRIDOR',370,310);
 for(const s of sprites){ // Move via the central corridor, so sprites use the room doors.
  if(Math.abs(s.y-s.ty)>2){if(Math.abs(s.x-450)>2){s.x+=Math.sign(450-s.x)*Math.min(3,Math.abs(450-s.x));}else{s.y+=Math.sign(s.ty-s.y)*Math.min(3,Math.abs(s.ty-s.y));}}else if(Math.abs(s.x-s.tx)>2){s.x+=Math.sign(s.tx-s.x)*Math.min(3,Math.abs(s.tx-s.x));}
  const x=Math.round(s.x),y=Math.round(s.y),walk=Math.abs(s.x-s.tx)>2||Math.abs(s.y-s.ty)>2,step=walk?Math.floor(performance.now()/150)%2:0;
  if(s.waiting){ctx.strokeStyle=s.color;ctx.lineWidth=2;ctx.strokeRect(x-12,y-20,24,38);}
  ctx.fillStyle=s.color;ctx.fillRect(x-6,y-17,12,11);ctx.fillStyle='#070a10';ctx.fillRect(x-8,y-5,16,16);ctx.fillRect(x-8,y+10,6,8+step*3);ctx.fillRect(x+2,y+10,6,11-step*3);ctx.fillStyle='white';ctx.fillRect(x-3,y-3,6,6);ctx.fillStyle=s.color;ctx.font='10px monospace';ctx.fillText(({planner:'P',worker:'W',router:'R',judge:'J'}[s.role]||'?'),x-3,y-25);ctx.fillText(s.id,x-25,y+35);
 }
 requestAnimationFrame(frame);
}
frame();
