// Lifecycle adapter: offline inspection shares the company process lifetime.
import {spawn} from 'node:child_process';
export function startModelSecurityService({spawnProcess=spawn,env=process.env,cwd}={}){
 const allowed=['PATH','LANG','LC_ALL','SYSTEMROOT'];
 const child=spawnProcess('python3',['scripts/model_security_scan.py','--watch'],{
  stdio:['ignore','inherit','inherit'],...(cwd?{cwd}:{}),env:Object.fromEntries(allowed.filter(k=>k in env).map(k=>[k,env[k]])),
 });
 child.on('error',()=>console.error('Offline model security scanner could not start; inspect local Python availability.'));
 child.on('exit',(code,signal)=>{if(code!==0&&!signal)console.error('Offline model security scanner stopped; model safety is not established.');});
 return {stop(signal='SIGTERM'){if(!child.killed)child.kill(signal);}};
}
