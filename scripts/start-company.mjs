import { spawn, spawnSync } from 'node:child_process';
import process from 'node:process';
import { createCompanyEnvLoader } from './company-env.mjs';

const office='http://127.0.0.1:8788',company='we-need-gents-lc';
const requestedMcpTransport=process.env.GENTS_MCP_TRANSPORT;
const reloadLocalEnv=createCompanyEnvLoader();
function loadLocalEnv(){reloadLocalEnv();if(requestedMcpTransport!==undefined)process.env.GENTS_MCP_TRANSPORT=requestedMcpTransport;if(!['stdio','sse'].includes(process.env.GENTS_MCP_TRANSPORT||'stdio'))throw new Error('GENTS_MCP_TRANSPORT must be stdio or sse');}
function run(command,args,options={}){
 const result=spawnSync(command,args,{cwd:process.cwd(),stdio:'inherit',...options});
 if(result.error)throw result.error;
 return result.status??1;
}
async function configuration(){
 try{const r=await fetch(office+'/api/config',{signal:AbortSignal.timeout(1500)});return r.ok?await r.json():null;}catch{return null;}
}
function openOffice(){
 if(process.env.GENTS_NO_OPEN==='1')return;
 const [command,args]=process.platform==='darwin'?['open',[]]:process.platform==='win32'?['cmd',['/c','start','']]:['xdg-open',[]];
 const child=spawn(command,[...args,office+'/?autoconnect=1'],{detached:true,stdio:'ignore'});
 child.on('error',()=>console.log('Open the office manually at '+office));child.unref();
}
function ensureCompany(){
 if(run('python3',['scripts/gents.py','--company',company,'status'],{stdio:'ignore'})!==0){
  if(run('python3',['scripts/gents.py','--company',company,'onboard'])!==0)throw new Error('Company onboarding failed');
 }
}
loadLocalEnv();
console.log('Checking the local Gents company runtime...');
if(run('python3',['scripts/gents.py','doctor'])!==0)process.exit(1);
const existing=await configuration();
if(existing){
 if(requestedMcpTransport){const r=await fetch(office+'/api/mcp/catalog',{signal:AbortSignal.timeout(1500)});const catalog=r.ok?await r.json():null;if(catalog?.harness_transport!==`local_${requestedMcpTransport}_mcp`)throw new Error(`The running office uses ${catalog?.harness_transport||'an older MCP runtime'}. Stop it in its terminal, then rerun with GENTS_MCP_TRANSPORT=${requestedMcpTransport}.`);}
 if(!existing.company_tools)throw new Error('An older office is already on port 8788. Stop it in its terminal, then run npm run company again.');
 ensureCompany();openOffice();console.log('The company is already running at '+office);process.exit(0);
}
let harness,inbox,awake,stopping=false,opened=false,failures=0;
function stop(signal='SIGTERM'){
 stopping=true;if(inbox&&!inbox.killed)inbox.kill('SIGTERM');if(harness&&!harness.killed)harness.kill(signal);
}
process.on('SIGINT',()=>stop('SIGINT'));process.on('SIGTERM',()=>stop());
try{
 while(!stopping){
  loadLocalEnv();
  harness=spawn('sh',['scripts/run-company.sh'],{cwd:process.cwd(),stdio:'inherit',env:{...process.env,GENTS_SUPERVISED:'1'}});
  const exited=new Promise((resolve,reject)=>{harness.once('exit',(code,signal)=>resolve({code,signal}));harness.once('error',reject);});
  if(process.platform==='darwin'){awake=spawn('caffeinate',['-i','-w',String(harness.pid)],{stdio:'ignore'});awake.on('error',()=>console.error('Could not start the keep-awake helper'));}
  const deadline=Date.now()+5*60_000;
  while(!(await configuration())){
   const result=await Promise.race([exited.then(value=>({exited:value})),new Promise(resolve=>setTimeout(()=>resolve(null),500))]);
   if(result?.exited)throw new Error('The company harness stopped before it became ready ('+(result.exited.signal||result.exited.code)+')');
   if(Date.now()>deadline)throw new Error('Company startup exceeded five minutes');
  }
  ensureCompany();
  if(process.env.GENTS_EMAIL_AUTO_START==='1'){
   inbox=spawn('python3',['scripts/company_inbox.py'],{cwd:process.cwd(),stdio:'inherit'});
   inbox.on('error',()=>console.error('Email request listener could not start; check local configuration'));
  }
  if(!opened){openOffice();opened=true;}
  console.log('\nThe Gents office is ready at '+office+'.');
  console.log('Keep this terminal open and the Mac connected to power.');
  console.log('The local executive handles blockers. Paid inference is disabled by the local company launcher.');
  console.log('Browser restarts reload .env.local and .env.company and recover saved work. Ctrl-C stops the office.\n');
  const result=await exited;
  if(inbox&&!inbox.killed)inbox.kill('SIGTERM');inbox=null;
  if(awake&&!awake.killed)awake.kill('SIGTERM');awake=null;
  if(result.code===75&&!stopping){console.log('Restarting the company with its retained checkpoints...');continue;}
  if(!stopping){
   // A clean, unsolicited exit is also downtime. Intentional shutdown signals
   // set stopping first; all other exits receive bounded restart backoff.
   failures++;
   const delay=Math.min(60_000,2_000*2**Math.min(failures-1,5));
   console.error(`Company runtime exited (${result.signal||result.code}); retrying in ${delay/1000}s with checkpoints retained.`);
   await new Promise(resolve=>setTimeout(resolve,delay));
   continue;
  }
  break;
 }
}catch(error){console.error(error.message);stop();process.exitCode=1;}
finally{if(awake&&!awake.killed)awake.kill('SIGTERM');if(inbox&&!inbox.killed)inbox.kill('SIGTERM');}
