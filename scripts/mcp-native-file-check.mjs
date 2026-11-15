// Opt-in native local-model -> governed Rust MCP -> workspace -> readback proof.
import {spawn} from 'node:child_process';
import {mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
const root=new URL('../',import.meta.url),origin='http://127.0.0.1:8796',company='mcp-native-files',run=randomUUID();
const output=new URL(`../data/mcp-native-file-check/${run}/`,import.meta.url);
await mkdir(output,{recursive:true,mode:0o700});
if(await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null))throw Error('Check port 8796 is already occupied; no listener was stopped');
const child=spawn('./target/debug/gents-harness',[],{cwd:root,stdio:['ignore','ignore','inherit'],env:{...process.env,AUTH_MODE:'local',BIND:'127.0.0.1:8796',CONFIG:'config/company.mcp-development.json',RUN_STORE:'data/mcp-native-file-check.sqlite3',LOCAL_MODEL_PORT:'8797',GENTS_DISABLE_SCHEDULER:'1',GENTS_LOCAL_ONLY:'1',GENTS_OFFICE_ACTIVITY:'0',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
let stopping=false;const stop=()=>{stopping=true;child.kill('SIGTERM');};process.on('SIGTERM',stop);process.on('SIGINT',stop);
async function request(path,data){const r=await fetch(origin+path,{method:data?'POST':'GET',headers:{'Content-Type':'application/json'},body:data?JSON.stringify(data):undefined,signal:AbortSignal.timeout(10000)});if(!r.ok)throw Error(`${r.status} ${await r.text()}`);return r.json();}
const op=(op,data={})=>request(`/api/companies/${company}`,{op,...data});
const report={status:'RUNNING',company,run,origin,started_at_ms:Date.now(),provider_cost_usd:0};
try{
 let ready=false;for(let i=0;i<100&&!stopping;i++){try{await request('/api/config');ready=true;break;}catch{await new Promise(r=>setTimeout(r,100));}}
 if(!ready)throw Error('Owned native check listener failed to start');
 await op('register',{name:'Native local Rust MCP file verification'});
 const task='Create native-proof.txt containing exactly READY. Read native-proof.txt back using files op=read to verify the exact content, then finish citing that actual readback. Use only files and finish. No shell, tests, memory tools, network, downloads, additional verification stages or questions are required. This is a bounded workspace file task, not a code-promotion request.';
 const criteria=['native-proof.txt contains exactly READY and was read back using files'];
 const state=await op('intake',{run_id:run,intake:{idea:task,acceptance:criteria.join('\n'),exclusions:'All tools except files and finish',timeline:'Bounded native integration check',company_signer:'Local research operator',provider_signer:'Local research operator',budget_usd:0,deliverable_type:'agent',required_mcp_calls:[{actor:'Engineer Gent',tool:'files'}]}});
 await op('approve',{run_id:run,digest:state.jobs.find(j=>j.run_id===run).digest});
 const planner={tools:[],skills:[],memory_from:[]},worker={tools:['files:write','files:read','finish'],skills:[],memory_from:[]};
 await op('governance_install',{run_id:run,expected_revision:0,policy:{revision:1,master:'PM Gent',ceilings:{'PM Gent':planner,'Engineer Gent':worker},grants:{'PM Gent':planner,'Engineer Gent':worker}}});
 await request('/api/runs',{company_id:company,run_id:run,task,consent:false,definition_of_done:criteria});
 console.log(JSON.stringify({...report,pid:child.pid,report:new URL('report.json',output).pathname}));
 await writeFile(new URL('report.json',output),JSON.stringify(report,null,2),{mode:0o600});
 let saved,last='';
 while(!stopping&&Date.now()-report.started_at_ms<600000){
  saved=await request(`/api/runs/${run}`);const q=await request(`/api/runs/${run}/attention`);
  const value=JSON.stringify({run,status:saved.status,events:saved.events.length,question:q.attention?.answer?null:q.attention?.question||null});if(value!==last){console.log(value);last=value;}
  if(saved.status!=='running')break;await new Promise(r=>setTimeout(r,5000));
 }
 if(saved?.status!=='completed')throw Error(`Native file check did not complete: ${saved?.status}`);
 const directory=new URL(`../data/companies/${company}/runs/${run}/`,import.meta.url);
 const body=await readFile(new URL('workspace/native-proof.txt',directory),'utf8');if(body!=='READY')throw Error('Workspace bytes differ from frozen contract');
 const journals=await Promise.all((await readdir(directory)).filter(n=>/^tool-\d+\.json$/.test(n)).map(async n=>JSON.parse(await readFile(new URL(n,directory),'utf8'))));
 for(const operation of ['write','read']){
  const journal=journals.find(j=>j.request.action==='files'&&j.request.arguments.op===operation&&j.result.result.ok===true&&j.result.result.mcp_receipt?.tool==='files');
  if(!journal)throw Error(`No actual native MCP ${operation} receipt`);
  if(operation==='read'&&journal.result.result.result.text!=='READY')throw Error('Native readback differs');
 }
 const calls=saved.events.filter(e=>e.reply);const charges=calls.reduce((sum,e)=>sum+(e.reply.usage.cost_usd||0),0);if(charges!==0)throw Error('Unexpected paid inference');
 Object.assign(report,{status:'PASS',elapsed_ms:Date.now()-report.started_at_ms,model_calls:calls.length,model_ids:[...new Set(calls.map(e=>e.model))],provider_cost_usd:charges,artifact_sha256:createHash('sha256').update(body).digest('hex'),native_write:true,native_readback:true,company_completion:true});
}catch(error){Object.assign(report,{status:'FAIL',error:String(error),elapsed_ms:Date.now()-report.started_at_ms});try{await request(`/api/runs/${run}/cancel`,{});}catch{};process.exitCode=1;}
finally{await writeFile(new URL('report.json',output),JSON.stringify(report,null,2),{mode:0o600});console.log(JSON.stringify(report));stop();}
