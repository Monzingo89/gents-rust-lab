// Resume/reverify one retained native team trial. No planning, approval, new run
// or inference API is issued by this observer. The harness uses local models.
import {spawn} from 'node:child_process';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url),folder=new URL(process.argv[2].replace(/\/?$/,'/'),root),store=new URL(process.argv[3],root),origin='http://127.0.0.1:8796';
const original=JSON.parse(await readFile(new URL('report.json',folder),'utf8')),trial=original.trials.at(-1),run=trial.run,session=trial.planning_session;
const plan=JSON.parse(await readFile(new URL(`${session}-planning.json`,folder),'utf8')),review=JSON.parse(await readFile(new URL(`${session}-execution-review.json`,folder),'utf8'));
const report={id:randomUUID(),original_report_status:original.status,run,same_run:true,status:'OBSERVING',synthetic_users:true,provider_cost_usd:0};
if(await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null))throw Error('Port occupied; no listener was stopped');
const child=spawn('./target/debug/gents-harness',[],{cwd:root,stdio:['ignore','ignore','inherit'],env:{...process.env,AUTH_MODE:'local',BIND:'127.0.0.1:8796',CONFIG:'config/company.mcp-development.json',RUN_STORE:store.pathname,LOCAL_MODEL_PORT:'8797',GENTS_DISABLE_SCHEDULER:'1',GENTS_LOCAL_ONLY:'1',GENTS_MCP_TRANSPORT:'sse',GENTS_OFFICE_ACTIVITY:'0',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
const request=async(path,data)=>{const r=await fetch(origin+path,{method:data?'POST':'GET',headers:{'Content-Type':'application/json'},body:data?JSON.stringify(data):undefined,signal:AbortSignal.timeout(180000)});if(!r.ok)throw Error(`${r.status}: ${await r.text()}`);return r.json();};
const canonical=value=>JSON.stringify(value,(_,v)=>v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.keys(v).sort().map(k=>[k,v[k]])):v);
const project=(v,p)=>p.slice(1).split('/').reduce((v,k)=>v[k.replaceAll('~1','/').replaceAll('~0','~')],v);
const operatorFile=await readFile(new URL('operator-inventory.json',folder),'utf8').catch(()=>null);
function oracle(tool,args){
 if(tool==='table_read_json'){assert.equal(args.path,'inventory.json');assert(operatorFile!==null);return {rows:JSON.parse(operatorFile),source_path:args.path,source_sha256:createHash('sha256').update(operatorFile).digest('hex'),source_bytes:Buffer.byteLength(operatorFile)};}
 if(tool==='table_aggregate'){
  const rows=args.rows.filter(row=>args.filter===null||typeof row[args.filter.field]===typeof args.filter.equals&&row[args.filter.field]===args.filter.equals),groups=new Map();
  for(const row of rows){assert(Number.isSafeInteger(row[args.value_field]));const key=JSON.stringify(row[args.group_by]),prior=groups.get(key)??{key:row[args.group_by],rows:0,total:0n};prior.rows++;prior.total+=BigInt(row[args.value_field]);groups.set(key,prior);}
  return {input_rows:args.rows.length,matched_rows:rows.length,excluded_rows:args.rows.length-rows.length,groups:[...groups].sort(([a],[b])=>a<b?-1:a>b?1:0).map(([,v])=>({...v,total:Number(v.total)})),total:Number(rows.reduce((sum,row)=>sum+BigInt(row[args.value_field]),0n)),exact:true};
 }
 throw Error('No independent oracle for this retained tool; observation cannot certify it');
}
let saved;
try{
 for(let attempt=0;;attempt++){try{await request('/api/config');break;}catch(e){if(child.exitCode!==null||attempt>99)throw e;await new Promise(r=>setTimeout(r,100));}}
 assert.equal((await request('/api/config')).local_only,true);
 saved=await request(`/api/runs/${run}`);report.before_status=saved.status;
 if(['cancelled','incomplete','failed'].includes(saved.status))await request(`/api/runs/${run}/resume`,{});
 let last='';
 for(;;){
  try{saved=await request(`/api/runs/${run}`);}catch(error){if(child.exitCode!==null)throw error;console.log(JSON.stringify({run,observation_error:String(error),action:'re-poll same live listener'}));await new Promise(r=>setTimeout(r,1000));continue;}
  if(saved.status!==last){console.log(JSON.stringify({run,status:saved.status}));last=saved.status;}
  if(saved.status!=='running')break;await new Promise(r=>setTimeout(r,2000));
 }
 assert.equal(saved.status,'completed');
 const directory=new URL(`data/companies/${original.company}/runs/${run}/`,root),journals=await Promise.all((await readdir(directory)).filter(n=>/^tool-\d+\.json$/.test(n)).map(async n=>JSON.parse(await readFile(new URL(n,directory),'utf8'))));
 if(operatorFile!==null)assert.equal(await readFile(new URL('workspace/inventory.json',directory),'utf8'),operatorFile,'operator source must remain unchanged');
 const calls=journals.filter(j=>j.result.result.ok===true),values=new Map(),required=new Map(),artifacts={};
 for(const binding of review.intake.artifact_bindings){
  const args=structuredClone(binding.arguments);for(const reference of binding.argument_sources??[])args[reference.field]=values.get(reference.path);
  const expectedResult=oracle(binding.source_tool,args),value=project(expectedResult,binding.pointer);values.set(binding.path,value);
  const key=canonical({actor:binding.source_actor,tool:binding.source_tool,args});required.set(key,{binding,args,expectedResult});
  const expected=binding.encoding==='json'?canonical(value):String(value),actual=await readFile(new URL('workspace/'+binding.path,directory),'utf8');assert.equal(actual,expected);artifacts[binding.path]=actual;
  const writes=calls.filter(j=>j.request.action==='files'&&j.request.arguments.op==='write'&&j.request.arguments.path===binding.path);assert.equal(writes.length,1);assert.equal(writes[0].result.result.mcp_receipt.actor,binding.writer);
  assert(calls.some(j=>j.request.action==='files'&&j.request.arguments.op==='read'&&j.request.arguments.path===binding.path&&j.result.result.result.text===expected&&j.result.result.mcp_receipt.actor===binding.writer));
 }
 for(const {binding,args,expectedResult} of required.values()){const matches=calls.filter(j=>j.request.action===binding.source_tool&&j.result.result.mcp_receipt?.actor===binding.source_actor&&canonical(j.request.arguments)===canonical(args));assert.equal(matches.length,1,'native source must not replay');assert.equal(canonical(matches[0].result.result.result),canonical(expectedResult));}
 const currentPlan=await request(`/api/companies/${original.company}/planning/${session}`),usage=[...saved.events.filter(e=>e.reply).map(e=>e.reply.usage),...(currentPlan.attempts??plan.messages.filter(m=>m.role==='ceo')).filter(a=>a.usage).map(a=>a.usage)];assert(usage.every(u=>u.cost_usd===0));
 Object.assign(report,{status:'PASS',artifacts,native_source_calls:required.size,source_replayed:false,artifact_writes_replayed:false,completed_local_replies:usage.length,local_input_tokens:usage.reduce((s,u)=>s+(u.input??0),0),local_output_tokens:usage.reduce((s,u)=>s+(u.output??0),0),local_token_scope:'measured run replies and every audited planning attempt; interrupted calls without usage remain unknown',unfinished_run_calls:saved.events.filter(e=>e.kind==='call_started').length-saved.events.filter(e=>e.reply).length,worker_model_calls:saved.events.filter(e=>e.role==='worker'&&e.kind==='call_started').length});
}catch(error){report.status='FAIL';report.error=String(error);process.exitCode=1;}
finally{
 await writeFile(new URL('same-run-resume-report.json',folder),JSON.stringify(report,null,2),{mode:0o600});if(saved)await writeFile(new URL('same-run-resume-events.json',folder),JSON.stringify(saved,null,2),{mode:0o600});
 const esc=v=>String(v).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');let html='<!doctype html><meta charset="utf-8"><title>Native composition same-run recovery</title><style>body{font:16px system-ui;max-width:1000px;margin:40px auto;background:#10242b;color:#d9eee5}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#08191f;padding:16px}</style><h1>Native composition same-run recovery</h1><p>The original failed report and interruption evidence are retained. The same approved UUID was resumed; no new planning, approval or task restart. Actual local model responses and public events follow. Scripted fake-user clarifications are explicit fixtures.</p><pre>'+esc(JSON.stringify(report,null,2))+'</pre>';
 for(const m of plan.messages)html+='<h3>'+esc(m.role==='user'?(m.kind==='planning_repair'?'Scripted fake user · planning repair':'Scripted fake user'):'Local CEO')+'</h3><pre>'+esc(typeof m.message==='string'?m.message:JSON.stringify(m.message,null,2))+'</pre>'+(m.host_compiled_readbacks?.length?'<p>Host-compiled readback controls: '+esc(JSON.stringify(m.host_compiled_readbacks))+'</p>':'');
 for(const e of saved?.events??[])html+='<h3>#'+e.seq+' '+esc(e.agent)+' / '+esc(e.kind)+'</h3><pre>'+esc(e.reply?.text||e.message||'')+'</pre>';
 await writeFile(new URL('same-run-resume-transcript.html',folder),html,{mode:0o600});console.log(JSON.stringify(report));
 if(saved?.status!=='running')child.kill('SIGTERM');else console.log('Live run retained; owned listener left running for continued observation');
}
