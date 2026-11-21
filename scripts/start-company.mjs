import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';
import {pathToFileURL} from 'node:url';
import {createCompanyEnvLoader} from './company-env.mjs';
import {startOwnedProcess,waitForReadiness,probeCompany} from './company-lifecycle.mjs';

export async function startCompany({root=process.cwd(),office='http://127.0.0.1:8788',company='we-need-gents-lc',
 env=process.env,signal,startupTimeoutMs=300000,shutdownTimeoutMs=5000,restartDelayMs=2000}={}){
 const runtimeEnv={...env},owned=new Set();
 const requestedMcpTransport=runtimeEnv.GENTS_MCP_TRANSPORT;
 const reload=createCompanyEnvLoader({root,env:runtimeEnv});
 const load=()=>{reload();if(requestedMcpTransport!==undefined)runtimeEnv.GENTS_MCP_TRANSPORT=requestedMcpTransport;if(!['stdio','sse'].includes(runtimeEnv.GENTS_MCP_TRANSPORT||'stdio'))throw new Error('GENTS_MCP_TRANSPORT must be stdio or sse');};
 const launch=(command,args,options={})=>{
  signal?.throwIfAborted();
  const handle=startOwnedProcess(command,args,{cwd:root,stdio:'inherit',env:runtimeEnv,...options});
  owned.add(handle);return handle;
 };
 const close=async handle=>{if(handle){await handle.stop('SIGTERM',shutdownTimeoutMs);owned.delete(handle);}};
 const abort=()=>{for(const handle of owned)void handle.stop('SIGTERM',shutdownTimeoutMs).catch(error=>console.error(error.message));};
 signal?.addEventListener('abort',abort,{once:true});
 async function run(command,args,options={}){
  const handle=launch(command,args,options);
  const timer=setTimeout(()=>handle.signal('SIGKILL'),60000);
  try{const result=await handle.exited;if(result.error)throw result.error;signal?.throwIfAborted();return result.code??1;}
  finally{clearTimeout(timer);await close(handle);}
 }
 async function ensureCompany(){
  if(await run('python3',['scripts/gents.py','--company',company,'status'],{stdio:'ignore'})!==0&&
     await run('python3',['scripts/gents.py','--company',company,'onboard'])!==0)throw new Error('Company onboarding failed');
 }
 function openOffice(){
  if(runtimeEnv.GENTS_NO_OPEN==='1')return;
  const [command,args]=process.platform==='darwin'?['open',[]]:process.platform==='win32'?['cmd',['/c','start','']]:['xdg-open',[]];
  const child=spawn(command,[...args,office+'/?autoconnect=1'],{detached:true,stdio:'ignore'});
  child.on('error',()=>console.log('Open the office manually at '+office));child.unref();
 }
 let opened=false,failures=0;
 try{
  load();console.log('Checking the local Gents company runtime...');
  if(await run('python3',['scripts/gents.py','doctor'])!==0)return 1;
  const probe=(probeSignal,startupId)=>probeCompany({origin:office,transport:runtimeEnv.GENTS_MCP_TRANSPORT||'stdio',startupId,signal:probeSignal});
  if(await probe(signal)){
   await ensureCompany();signal?.throwIfAborted();openOffice();console.log('The company is already running at '+office);return 0;
  }
  for(;;){
   signal?.throwIfAborted();load();
   const startupId=randomUUID();
   const harness=launch('sh',['scripts/run-company.sh'],{env:{...runtimeEnv,GENTS_SUPERVISED:'1',GENTS_SUPERVISOR_PID:String(process.pid),GENTS_STARTUP_ID:startupId}});
   let awake,inbox;
   try{
    if(process.platform==='darwin'){
     awake=launch('caffeinate',['-i','-w',String(harness.child.pid)],{stdio:'ignore',ownGroup:false});
     awake.child.once('error',()=>console.error('Could not start the keep-awake helper'));
    }
    await waitForReadiness({owned:harness,probe:probeSignal=>probe(probeSignal,startupId),signal,timeoutMs:startupTimeoutMs});
    await ensureCompany();signal?.throwIfAborted();
    if(runtimeEnv.GENTS_EMAIL_AUTO_START==='1'){
     inbox=launch('python3',['scripts/company_inbox.py']);
     inbox.child.once('error',()=>console.error('Email request listener could not start; check local configuration'));
    }
    if(!opened){openOffice();opened=true;}
    console.log('\nThe Gents office control plane and native bundle are ready at '+office+'.');
    console.log('Keep this terminal open. Models load on demand; readiness does not establish task completion.');
    console.log('Paid inference is disabled by the local company launcher. Ctrl-C stops its owned processes.\n');
    const result=await harness.exited;
    if(result.error)throw result.error;
    signal?.throwIfAborted();
    if(result.code===75){console.log('Restarting the company with its retained checkpoints...');continue;}
    failures++;
    const backoff=Math.min(60000,restartDelayMs*2**Math.min(failures-1,5));
    console.error(`Company runtime exited (${result.signal||result.code}); retrying in ${backoff/1000}s with checkpoints retained.`);
    // Cleanup precedes backoff: an exited wrapper must not leave a second runtime.
    await close(inbox);inbox=null;await close(awake);awake=null;await close(harness);
    await delay(backoff,undefined,{signal});
   }finally{await close(inbox);await close(awake);await close(harness);}
  }
 }catch(error){if(signal?.aborted)return 0;console.error(error.message);return 1;}
 finally{signal?.removeEventListener('abort',abort);await Promise.all([...owned].map(close));}
}

if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const controller=new AbortController();
 const stop=()=>controller.abort(new Error('Company launcher stopped'));
 process.on('SIGINT',stop);process.on('SIGTERM',stop);
 try{process.exitCode=await startCompany({signal:controller.signal});}
 finally{process.off('SIGINT',stop);process.off('SIGTERM',stop);}
}
