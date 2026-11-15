// Dedicated local-only presentation and efficiency observer. Never starts old
// company jobs, cloud escalation, downloads, email or the general scheduler.
import {spawn} from 'node:child_process';
const root=new URL('../',import.meta.url),origin='http://127.0.0.1:8788';
let child,awake,stopping=false;
function stop(){stopping=true;child?.kill('SIGTERM');awake?.kill('SIGTERM');}
process.on('SIGINT',stop);process.on('SIGTERM',stop);
try {
 const existing=await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null);
 if(existing?.ok)throw new Error('Port 8788 already has a harness. Stop that listener intentionally before starting this isolated office activity service.');
 while(!stopping){
  child=spawn('./target/debug/gents-harness',[],{cwd:root,stdio:'inherit',env:{...process.env,AUTH_MODE:'local',BIND:'127.0.0.1:8788',CONFIG:'config/company.local.json',RUN_STORE:'data/office-view.sqlite3',GENTS_DISABLE_SCHEDULER:'1',GENTS_LOCAL_ONLY:'1',GENTS_OFFICE_ACTIVITY:'1',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
  if(process.platform==='darwin'){awake=spawn('caffeinate',['-i','-w',String(child.pid)],{stdio:'ignore'});awake.on('error',()=>console.warn('Keep-awake helper unavailable.'));}
  console.log('Office activity: '+origin+'/?autoconnect=1&tour=1');
  console.log('Local observer runs continuously; evidence-driven model reviews are capped at one attempt per three hours. Ctrl-C stops this service.');
  const result=await new Promise((resolve,reject)=>{child.once('exit',(code,signal)=>resolve({code,signal}));child.once('error',reject);});
  awake?.kill('SIGTERM');awake=null;
  if(!stopping){console.error(`Office listener exited (${result.signal||result.code}); restarting in 10 seconds with the saved notebook.`);await new Promise(resolve=>setTimeout(resolve,10000));}
 }
} catch(error){console.error(error.message);stop();process.exitCode=1;}
