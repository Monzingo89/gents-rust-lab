import { spawn } from 'node:child_process';
import { startMobileAccess } from './mobile-access.mjs';

const office='http://127.0.0.1:8788';let company,awake,mobile,stopping=false;
async function configuration(){try{const r=await fetch(office+'/api/config',{signal:AbortSignal.timeout(1500)});return r.ok?await r.json():null;}catch{return null;}}
async function stop(){if(stopping)return;stopping=true;await mobile?.close();if(awake&&!awake.killed)awake.kill('SIGTERM');if(company&&!company.killed)company.kill('SIGTERM');}
process.on('SIGINT',stop);process.on('SIGTERM',stop);
try{
 let config=await configuration();
 if(!config){
  company=spawn(process.execPath,['scripts/start-company.mjs'],{stdio:'inherit',env:{...process.env,GENTS_NO_OPEN:'1'}});
  let exited=false;company.once('exit',()=>{exited=true;if(mobile&&!stopping){console.error('The laptop company stopped. Closing mobile access.');stop();}});
  company.once('error',()=>{exited=true;});
  const deadline=Date.now()+300_000;
  while(!stopping&&!(config=await configuration())){if(exited||Date.now()>deadline)throw new Error('The laptop company could not start. Check its terminal output.');await new Promise(resolve=>setTimeout(resolve,500));}
 }
 if(!stopping){
  if(!config?.company_tools||config.auth!=='local')throw new Error('Port 8788 must be the local-auth Gents company runtime');
  mobile=await startMobileAccess();
  if(process.platform==='darwin'){awake=spawn('caffeinate',['-i','-w',String(process.pid)],{stdio:'ignore'});awake.on('error',()=>console.error('Keep the laptop awake manually; the keep-awake helper could not start.'));}
  console.log(`Mobile office (same Wi-Fi only): ${mobile.url}`);
  console.log(`Open ${mobile.setupUrl} ON THIS LAPTOP for the pairing code and phone instructions.`);
  console.log('All inference stays on this laptop. Keep this terminal running, power connected, and the lid open.');
  console.log('Ctrl-C closes phone access and revokes its sessions. An already-running company is left running.');
  if(process.env.GENTS_NO_OPEN!=='1'){
   const [command,args]=process.platform==='darwin'?['open',[]]:process.platform==='win32'?['cmd',['/c','start','']]:['xdg-open',[]];
   const browser=spawn(command,[...args,mobile.setupUrl],{stdio:'ignore',detached:true});browser.on('error',()=>console.error('Open the laptop setup address manually.'));browser.unref();
  }
 }
}catch(error){console.error(error.message);await stop();process.exitCode=1;}
