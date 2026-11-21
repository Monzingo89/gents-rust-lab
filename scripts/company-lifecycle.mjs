// Trusted construction boundary for processes owned by a local launcher.
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';

export function startOwnedProcess(command,args,{ownGroup=true,...options}={}){
 const grouped=ownGroup&&process.platform!=='win32';
 const child=spawn(command,args,{...options,detached:grouped});
 let result,stopping;
 const exited=new Promise(resolve=>{
  const finish=value=>{if(result===undefined){result=value;resolve(value);}};
  child.once('error',error=>finish({code:null,signal:null,error}));
  child.once('exit',(code,signal)=>finish({code,signal}));
 });
 const spawned=new Promise(resolve=>{child.once('spawn',resolve);child.once('error',resolve);});
 function send(signal){
  if(!child.pid)return false;
  try{if(grouped)process.kill(-child.pid,signal);else child.kill(signal);return true;}
  catch(error){if(error.code==='ESRCH')return false;error.message+=` (owned ${command}, pid ${child.pid}, group ${grouped}, exited ${result!==undefined})`;throw error;}
 }
 function alive(){
  if(!grouped)return result===undefined;
  return send(0);
 }
 return {child,exited,get result(){return result;},signal:send,
  stop(signal='SIGTERM',graceMs=5000){
   return stopping??=(async()=>{
    // Node can expose a PID before the detached child has finished launching.
    // Wait for spawn/error before attempting to address its process group.
    await spawned;
    send(signal);
    const deadline=performance.now()+graceMs;
    while(alive()&&performance.now()<deadline)await delay(Math.min(25,Math.max(1,deadline-performance.now())));
    const forced=alive();
    if(forced)send('SIGKILL');
    await exited;
    return {...result,forced};
   })();
  }
 };
}

// A deadline or cancellation wins even when a probe ignores its signal.
export async function waitForReadiness({owned,probe,signal,timeoutMs=300000,pollMs=500}){
 const controller=new AbortController();
 const abort=()=>controller.abort(signal.reason);
 signal?.addEventListener('abort',abort,{once:true});
 if(signal?.aborted)abort();
 const timer=setTimeout(()=>controller.abort(new Error('Company startup exceeded its readiness deadline')),timeoutMs);
 let rejectAbort;
 const aborted=new Promise((_,reject)=>{rejectAbort=()=>reject(controller.signal.reason);controller.signal.addEventListener('abort',rejectAbort,{once:true});if(controller.signal.aborted)rejectAbort();});
 const stopped=owned.exited.then(result=>{throw result.error||new Error(`Company process stopped before readiness (${result.signal||result.code})`);});
 try{
  return await Promise.race([aborted,stopped,(async()=>{
   for(;;){
    controller.signal.throwIfAborted();
    const value=await probe(controller.signal);
    controller.signal.throwIfAborted();
    if(owned.result)throw owned.result.error||new Error('Company process exited during readiness');
    if(value)return value;
    await delay(pollMs,undefined,{signal:controller.signal});
   }
  })()]);
 }finally{
  clearTimeout(timer);
  signal?.removeEventListener('abort',abort);
  controller.signal.removeEventListener('abort',rejectAbort);
  controller.abort();
 }
}

// This checks the control plane and sealed native bundle, not model residency.
export async function probeCompany({origin,transport='stdio',startupId,signal}){
 const url=new URL(origin);
 if(url.protocol!=='http:'||!['127.0.0.1','[::1]'].includes(url.hostname)||url.username||url.password||url.pathname!=='/'||url.search||url.hash)throw new Error('Company readiness requires a credential-free loopback HTTP origin');
 const requestSignal=AbortSignal.any([...(signal?[signal]:[]),AbortSignal.timeout(1500)]);
 let response;
 try{response=await fetch(origin+'/api/config',{signal:requestSignal,redirect:'error'});}
 catch(error){if(signal?.aborted)throw signal.reason;if(error.name==='TimeoutError'||error.cause?.code==='ECONNREFUSED')return null;throw error;}
 if(!response.ok)throw new Error('The office address is occupied by an incompatible service; no process was stopped');
 const config=await response.json();
 if(config.auth!=='local'||config.local_only!==true||config.company_mode!==true||config.company_tools!==true)throw new Error('The office requires local authentication, local-only inference and company tools');
 if(startupId&&config.startup_id!==startupId)throw new Error('The office listener is not the process instance launched here');
 const catalogResponse=await fetch(origin+'/api/mcp/catalog',{signal:requestSignal,redirect:'error'});
 if(!catalogResponse.ok)throw new Error('The office native catalog is unavailable');
 const catalog=await catalogResponse.json();
 if(catalog.harness_transport!==`local_${transport}_mcp`)throw new Error(`The office uses ${catalog.harness_transport||'an unknown transport'}; expected local_${transport}_mcp`);
 const binding=catalog.runtime_binding;
 if(binding?.supported!==true||binding.mode!=='sealed_bundle'||binding.bundle_verified!==true)throw new Error('The office has not verified its native runtime bundle; rebuild and restart it intentionally');
 return config;
}
