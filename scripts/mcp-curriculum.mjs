// Bounded synthetic local company curriculum; no assistant/API inference calls.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,readdir} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
const useExisting=process.argv.includes('--existing');
const root=new URL('../',import.meta.url),origin=useExisting?'http://127.0.0.1:8788':'http://127.0.0.1:8796',company='mcp-local-curriculum';
const output=new URL(`../data/mcp-curriculum/${randomUUID()}/`,import.meta.url);
await mkdir(output,{recursive:true,mode:0o700});
if(!useExisting&&await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null))throw Error('Port 8796 occupied; no listener stopped');
const child=useExisting?null:spawn('./target/debug/gents-harness',[],{cwd:root,stdio:['ignore','ignore','inherit'],env:{...process.env,AUTH_MODE:'local',BIND:'127.0.0.1:8796',CONFIG:'config/company.mcp-development.json',RUN_STORE:'data/mcp-curriculum.sqlite3',LOCAL_MODEL_PORT:'8797',GENTS_DISABLE_SCHEDULER:'1',GENTS_OFFICE_ACTIVITY:'0',GENTS_LOCAL_ONLY:'1',GENTS_MCP_TRANSPORT:process.env.GENTS_MCP_TRANSPORT||'sse',GENTS_OFFICE_ACTIVITY:'0',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
let active,activeSession,keepListener=false;const stop=()=>child?.kill('SIGTERM');process.on('SIGTERM',stop);process.on('SIGINT',stop);
async function request(path,data){const r=await fetch(origin+path,{method:data?'POST':'GET',headers:{'Content-Type':'application/json'},body:data?JSON.stringify(data):undefined,signal:AbortSignal.timeout(180000)});if(!r.ok){const error=Error(`${r.status}: ${await r.text()}`);error.status=r.status;throw error;}return r.json();}
const op=(op,data={})=>request(`/api/companies/${company}`,{op,...data});
const cases=[
 {id:'01-exact-file',idea:'Create a greeting file for me.',spec:'Create greeting.txt containing exactly HELLO with no newline. Read it back with files op=read. The exact bytes and successful native write/readback are the complete definition of done. Assign only Engineer Gent using files:write, files:read and finish. No code, shell, tests, memory or extra verification is required.',files:{'greeting.txt':'HELLO'},tools:['files:write','files:read','finish']},
 {id:'02-multiple-files',idea:'Create a pair of status files for me.',spec:'Create alpha.txt containing exactly READY and beta.txt containing exactly WAIT, both with no newline. Read both files back using files op=read. The two exact contents and native write/readback are the complete definition of done. Assign only Engineer Gent using files:write, files:read and finish. No shell, code, tests, memory or extra verification is required.',files:{'alpha.txt':'READY','beta.txt':'WAIT'},tools:['files:write','files:read','finish']},
 {id:'03-arithmetic-to-artifact',idea:'Calculate a total and save the result for me.',spec:'Use integer_arithmetic with a calculation id total, values [17,28,-3], subtract 0. Create total.txt containing exactly 42 with no newline, then read it back using files op=read. Successful native arithmetic plus exact artifact bytes and native readback are the complete definition of done. Assign only Engineer Gent using integer_arithmetic, files:write, files:read and finish. No shell, code, tests, memory or extra verification is required.',files:{'total.txt':'42'},tools:['integer_arithmetic','files:write','files:read','finish']},
];
cases.push({id:'04-outcome-only-routing',idea:'Calculate a balance and save it locally for me.',spec:'Add the signed integers [125,-38,17], then subtract 4. Save only the resulting integer as plain text to ledger.txt with no newline. Verify the stored contents by reading the file back, then stop. Choose the minimum staffing and capabilities from the installed catalog. All work stays in the local workspace. No code, shell, external research, memory or additional verification is required.',files:{'ledger.txt':'100'},tools:['integer_arithmetic','files:write','files:read','finish']});
const report={status:'RUNNING',started_at_ms:Date.now(),synthetic_users:true,owned_listener:!useExisting,transport:process.env.GENTS_MCP_TRANSPORT||'sse',trials:[],provider_cost_usd:0};
async function save(){await writeFile(new URL('report.json',output),JSON.stringify(report,null,2),{mode:0o600});}
try{
 let ready=false;for(let i=0;i<100;i++){try{await request('/api/config');ready=true;break;}catch{await new Promise(r=>setTimeout(r,100));}}if(!ready)throw Error('Owned curriculum listener failed to start');
 const runtimeConfig=await request('/api/config');report.transport=(await request('/api/mcp/catalog')).harness_transport;if(runtimeConfig.local_only!==true||!runtimeConfig.company_tools||runtimeConfig.agents.some(a=>a.planner_provider!=='local'||a.worker_provider!=='local'))throw Error('Curriculum requires an explicitly local-only native company runtime');
 await op('register',{name:'Synthetic local MCP curriculum'});console.log(JSON.stringify({status:'STARTED',report:new URL('report.json',output).pathname,origin,pid:child?.pid??null}));await save();
 const selectedId=process.argv.includes('--case')?process.argv[process.argv.indexOf('--case')+1]:null;
 const selectedCases=selectedId?cases.filter(test=>test.id===selectedId):cases;
 if(selectedCases.length===0)throw Error('Unknown curriculum case');
 for(const test of selectedCases){
  const session=randomUUID(),trial={case:test.id,planning_session:session,status:'PLANNING'};report.trials.push(trial);activeSession=session;await save();
  let plan=await request(`/api/companies/${company}/planning/${session}`,{message:test.idea,expected_revision:0});
  // This is an explicit scripted fake user, not an invented model conversation.
  for(let turn=0;plan.status==='question'&&turn<3;turn++)plan=await request(`/api/companies/${company}/planning/${session}`,{message:test.spec,expected_revision:plan.revision});
  await writeFile(new URL(`${test.id}-planning.json`,output),JSON.stringify(plan,null,2),{mode:0o600});
  if(plan.status!=='ready')throw Error(`CEO planning ended ${plan.status}: ${plan.error||''}`);
  const assignments=plan.proposal.assignments;
  if(assignments.length!==1||assignments[0].actor!=='Engineer Gent'||assignments[0].tools.some(t=>!test.tools.includes(t))||test.tools.filter(t=>t!=='finish').some(t=>!assignments[0].tools.includes(t)))throw Error('CEO staffing/tool proposal differs from this frozen experiment ceiling; no authority installed');
  const run=randomUUID();active=run;trial.run=run;trial.host_completion_control_added=!assignments[0].tools.includes('finish');trial.status="EXECUTING";await save();
  const task=test.spec+'\nLOCAL CEO PLAN (proposal only): '+JSON.stringify(plan.proposal)+'\nUse only the assigned tools. The scripted user has supplied all requirements.';
  const criteria=plan.proposal.definition_of_done;
  const required=[{actor:'Engineer Gent',tool:'files'},...(test.tools.includes('integer_arithmetic')?[{actor:'Engineer Gent',tool:'integer_arithmetic'}]:[])];
  const draft=await op('intake',{run_id:run,intake:{idea:task,acceptance:criteria.join('\n'),exclusions:'All remote/paid services, shell, downloads, secrets, permission changes and tools outside the proposed experiment ceiling',timeline:'Bounded synthetic local curriculum',company_signer:'Local research operator',provider_signer:'Local research operator',budget_usd:0,deliverable_type:'agent',required_mcp_calls:required,ceo_planning_id:session,ceo_planning_revision:plan.revision,ceo_plan:plan.proposal}});
  await op('approve',{run_id:run,digest:draft.jobs.find(j=>j.run_id===run).digest});
  const empty={tools:[],skills:[],memory_from:[]},worker={tools:[...new Set([...assignments[0].tools,'finish'])],skills:[],memory_from:[]};
  await op('governance_install',{run_id:run,expected_revision:0,policy:{revision:1,master:'PM Gent',ceilings:{'PM Gent':empty,'Engineer Gent':worker},grants:{'PM Gent':empty,'Engineer Gent':worker}}});
  await request('/api/runs',{company_id:company,run_id:run,task,consent:false,definition_of_done:criteria});
  let saved,last='';const deadline=Date.now()+300000;
  while(true){
   if(Date.now()>deadline&&!trial.observation_window_elapsed){trial.observation_window_elapsed=true;await save();console.log(JSON.stringify({run,status:'OBSERVATION_WINDOW_ELAPSED',action:'continue observing the same UUID without cancellation'}));}
   try{saved=await request(`/api/runs/${run}`);}catch(error){if(child&&child.exitCode!==null||error.status===404)throw error;trial.last_observation_error=String(error);await save();await new Promise(r=>setTimeout(r,1000));continue;}
   const next=JSON.stringify({case:test.id,run,status:saved.status,events:saved.events.length});if(next!==last){console.log(next);last=next;}if(saved.status!=='running')break;await new Promise(r=>setTimeout(r,3000));
  }
  await writeFile(new URL(`${test.id}-events.json`,output),JSON.stringify(saved,null,2),{mode:0o600});
  if(saved.status!=='completed')throw Error(`Trial ${test.id} did not complete: ${saved.status}`);
  const directory=new URL(`../data/companies/${company}/runs/${run}/`,import.meta.url);
  const journals=await Promise.all((await readdir(directory)).filter(n=>/^tool-\d+\.json$/.test(n)).map(async n=>JSON.parse(await readFile(new URL(n,directory),'utf8'))));
  for(const [path,expected] of Object.entries(test.files)){
   const actual=await readFile(new URL('workspace/'+path,directory),'utf8');if(actual!==expected)throw Error(`${path} exact byte mismatch`);
   for(const operation of ['write','read'])if(!journals.some(j=>j.request.action==='files'&&j.request.arguments.op===operation&&j.request.arguments.path===path&&j.result.result.ok===true&&j.result.result.mcp_receipt?.tool==='files'&&(operation!=='read'||j.result.result.result.text===expected)))throw Error(`Missing actual MCP ${operation} for ${path}`);
  }
  if(test.tools.includes('integer_arithmetic')&&!journals.some(j=>j.request.action==='integer_arithmetic'&&j.result.result.ok===true&&j.result.result.mcp_receipt?.tool==='integer_arithmetic'&&j.result.result.result.calculations.some(c=>c.difference===Number(Object.values(test.files)[0]))))throw Error('Missing correct actual native arithmetic receipt');
  const replies=saved.events.filter(e=>e.reply),planning=plan.messages.filter(m=>m.role==='ceo');
  const charges=[...replies.map(e=>e.reply.usage),...planning.map(m=>m.usage)].reduce((sum,u)=>sum+(u.cost_usd??0),0);if(charges!==0)throw Error('Unexpected provider charge');
  Object.assign(trial,{status:'PASS',model_calls:replies.length+planning.length,local_input_tokens:[...replies.map(e=>e.reply.usage),...planning.map(m=>m.usage)].reduce((sum,u)=>sum+(u.input??0),0),local_output_tokens:[...replies.map(e=>e.reply.usage),...planning.map(m=>m.usage)].reduce((sum,u)=>sum+(u.output??0),0),provider_cost_usd:charges,artifacts:Object.fromEntries(Object.entries(test.files).map(([p,v])=>[p,createHash('sha256').update(v).digest('hex')]))});active=null;activeSession=null;await save();
 }
 report.status='PASS';
}catch(error){report.status='FAIL';report.error=String(error);if(report.trials.length)report.trials.at(-1).status='FAIL';process.exitCode=1;
 if(active){try{const current=await request(`/api/runs/${active}`);keepListener=current.status==='running';}catch(observation){keepListener=observation.status!==404&&(!child||child.exitCode===null);}}
 else if(activeSession){try{const plan=await request(`/api/companies/${company}/planning/${activeSession}`);keepListener=plan.status==='generating';}catch(observation){keepListener=observation.status!==404&&(!child||child.exitCode===null);}}
 if(keepListener)report.status='OBSERVATION_ERROR_LIVE_WORK_RETAINED';
}
finally{
 report.elapsed_ms=Date.now()-report.started_at_ms;await save();
 const esc=value=>String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');
 let html='<!doctype html><meta charset="utf-8"><title>Local company curriculum transcript</title><style>body{font:16px system-ui;max-width:1000px;margin:40px auto;background:#10242b;color:#d9eee5}pre{white-space:pre-wrap;overflow-wrap:anywhere;padding:16px;background:#08191f}article{border-top:1px solid #53766b;margin-top:24px}</style><h1>Synthetic local company curriculum</h1><p>Scripted fake users; actual local model responses, host events and native tool receipts. Local inference consumes tokens; provider charges are zero. No hidden reasoning is included.</p><pre>'+esc(JSON.stringify(report,null,2))+'</pre>';
 for(const trial of report.trials){
  html+='<h2>'+esc(trial.case)+'</h2>';
  try{const plan=JSON.parse(await readFile(new URL(`${trial.case}-planning.json`,output),'utf8'));for(const m of plan.messages)html+='<article><h3>'+esc(m.role==='user'?'Scripted fake user':'Local CEO')+'</h3><pre>'+esc(typeof m.message==='string'?m.message:JSON.stringify(m.message,null,2))+'</pre></article>';}catch{}
  try{const run=JSON.parse(await readFile(new URL(`${trial.case}-events.json`,output),'utf8'));for(const e of run.events)html+='<article><h3>#'+e.seq+' '+esc(e.agent)+' / '+esc(e.kind)+'</h3><pre>'+esc(e.reply?.text||e.message||'')+'</pre></article>';}catch{}
 }
 await writeFile(new URL('transcript.html',output),html,{mode:0o600});console.log(JSON.stringify(report));if(!keepListener)stop();else console.log(JSON.stringify({run:active,pid:child?.pid??null,action:'retain live work; no timeout cancellation'}));
}
