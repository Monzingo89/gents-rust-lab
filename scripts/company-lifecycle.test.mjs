import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import {createServer} from 'node:net';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {startOwnedProcess,waitForReadiness,probeCompany} from './company-lifecycle.mjs';
import {startCompany} from './start-company.mjs';

const fixture=fileURLToPath(new URL('fixtures/company-lifecycle.mjs',import.meta.url));
const localEnv=()=>Object.fromEntries(['PATH','HOME','TMPDIR','LANG'].filter(k=>k in process.env).map(k=>[k,process.env[k]]));
async function until(check,ms=5000){
 const deadline=performance.now()+ms;
 for(;;){try{const result=await check();if(result)return result;}catch(error){if(error.code!=='ENOENT')throw error;}
  assert.ok(performance.now()<deadline,'fixture observation timed out');await delay(20);
 }
}
async function temporary(t){const folder=await mkdtemp(join(tmpdir(),'gents-lifecycle-'));t.after(()=>rm(folder,{recursive:true,force:true}));return folder;}
async function freePort(){const server=createServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const port=server.address().port;await new Promise(resolve=>server.close(resolve));return port;}
function launch(mode,state,port){return startOwnedProcess(process.execPath,[fixture],{stdio:'ignore',env:{...localEnv(),GENTS_FIXTURE_MODE:mode,GENTS_FIXTURE_STATE:state,GENTS_FIXTURE_PORT:String(port),GENTS_STARTUP_ID:'expected-instance'}});}

test('startup cancellation interrupts a hung probe and cleans its process group',{skip:process.platform==='win32'},async t=>{
 const folder=await temporary(t),state=join(folder,'cancel');const owned=launch('never-ready',state,0);
 t.after(()=>owned.stop('SIGTERM',100));await until(()=>readFile(state+'.waiting'));
 const controller=new AbortController();
 const waiting=waitForReadiness({owned,signal:controller.signal,probe:()=>new Promise(()=>{}),timeoutMs:10000});
 controller.abort(new Error('owner cancelled startup'));
 await assert.rejects(waiting,/owner cancelled startup/);
 const result=await owned.stop('SIGTERM',100);assert.equal(result.forced,true);
 assert.equal(result.signal,'SIGKILL');
});
test('immediate shutdown waits for spawn and repeated stop calls share cleanup',{skip:process.platform==='win32',timeout:3000},async t=>{
 const folder=await temporary(t);
 for(let i=0;i<3;i++){
  const owned=launch('never-ready',join(folder,String(i)),0);t.after(()=>owned.stop('SIGTERM',100));
  const first=owned.stop('SIGTERM',100);
  assert.equal(owned.stop(),first);
  const result=await first;assert.equal(result.signal,'SIGTERM');
 }
});
test('readiness deadline and early process exit cannot be reported ready',async t=>{
 const folder=await temporary(t),owned=launch('never-ready',join(folder,'deadline'),0);
 t.after(()=>owned.stop('SIGTERM',100));
 await assert.rejects(waitForReadiness({owned,probe:()=>new Promise(()=>{}),timeoutMs:50}),/readiness deadline/);
 const early=launch('exit-before-ready',join(folder,'early'),0);t.after(()=>early.stop());
 await assert.rejects(waitForReadiness({owned:early,probe:async()=>null,pollMs:10,timeoutMs:1000}),/stopped before readiness \(9\)/);
 const absent=startOwnedProcess(join(folder,'absent-executable'),[],{stdio:'ignore'});t.after(()=>absent.stop());
 await assert.rejects(waitForReadiness({owned:absent,probe:async()=>null,timeoutMs:1000}),/ENOENT/);
});
test('shutdown handles a transient group-reaping denial but reports persistent permission errors',{skip:process.platform==='win32'},async t=>{
 const folder=await temporary(t),owned=launch('never-ready',join(folder,'reap'),0);
 await until(()=>readFile(join(folder,'reap.waiting')));
 const kill=process.kill.bind(process);let denials=1;
 const mocked=t.mock.method(process,'kill',(pid,signal)=>{
  if(pid===-owned.child.pid&&denials-->0)throw Object.assign(new Error('fixture permission denial'),{code:'EPERM'});
  return kill(pid,signal);
 });
 assert.equal((await owned.stop('SIGKILL',100)).signal,'SIGKILL');
 const blocked=launch('never-ready',join(folder,'denied'),0);
 await until(()=>readFile(join(folder,'denied.waiting')));
 mocked.mock.mockImplementation((pid,signal)=>{
  if(pid===-blocked.child.pid)throw Object.assign(new Error('persistent fixture permission denial'),{code:'EPERM'});
  return kill(pid,signal);
 });
 await assert.rejects(blocked.stop('SIGTERM',100),/persistent fixture permission denial/);
 mocked.mock.restore();kill(-blocked.child.pid,'SIGKILL');await blocked.exited;
});
test('group shutdown kills an orphan descendant while an unrelated listener survives',{skip:process.platform==='win32'},async t=>{
 const folder=await temporary(t),state=join(folder,'orphan');const parent=launch('orphan-parent',state,0),other=launch('ready',join(folder,'other'),0);
 t.after(()=>Promise.all([parent.stop('SIGTERM',100),other.stop('SIGTERM',100)]));
 const child=await until(async()=>JSON.parse(await readFile(state+'.listener','utf8')));
 const unrelated=await until(async()=>JSON.parse(await readFile(join(folder,'other.listener'),'utf8')));
 assert.ok((await fetch(`http://127.0.0.1:${child.port}/api/config`)).ok);
 const result=await parent.stop('SIGTERM',150);assert.equal(result.forced,true);
 await until(async()=>{try{await fetch(`http://127.0.0.1:${child.port}/api/config`,{signal:AbortSignal.timeout(100)});return false;}catch{return true;}});
 assert.ok((await fetch(`http://127.0.0.1:${unrelated.port}/api/config`)).ok);
 assert.equal(other.result,undefined);
});
test('readiness checks launched identity, transport and verified native bundle',async t=>{
 const folder=await temporary(t);
 for(const [mode,pattern] of [['foreign-instance',/not the process instance/],['unsealed',/not verified/],['cloud-enabled',/local-only inference/],['ready',null]]){
  const state=join(folder,mode),owned=launch(mode,state,0);t.after(()=>owned.stop());
  const {port}=await until(async()=>JSON.parse(await readFile(state+'.listener','utf8')));
  const options={origin:`http://127.0.0.1:${port}`,startupId:'expected-instance'};
  if(pattern)await assert.rejects(probeCompany(options),pattern);
  else{assert.equal((await probeCompany(options)).startup_id,'expected-instance');await assert.rejects(probeCompany({...options,transport:'sse'}),/expected local_sse_mcp/);}
  await owned.stop();
 }
 await assert.rejects(probeCompany({origin:'https://example.com'}),/loopback/);
});

async function supervisorFixture(t,mode){
 const root=await temporary(t),state=join(root,'runtime'),port=await freePort();
 await mkdir(join(root,'scripts'));
 await writeFile(join(root,'scripts/gents.py'),'import os,sys,time,signal\nopen(os.environ["GENTS_FIXTURE_STATE"]+".commands","a").write(sys.argv[-1]+"\\n")\nif sys.argv[-1]=="doctor" and os.environ.get("GENTS_FIXTURE_DOCTOR_WAIT")=="1":\n signal.signal(signal.SIGTERM,signal.SIG_IGN)\n open(os.environ["GENTS_FIXTURE_STATE"]+".doctor","w").write("waiting")\n time.sleep(60)\nsys.exit(3 if sys.argv[-1]=="status" and os.environ.get("GENTS_FIXTURE_ONBOARD_FAIL")=="1" else 4 if sys.argv[-1]=="onboard" else 0)\n');
 await writeFile(join(root,'scripts/run-company.sh'),'exec "$GENTS_FIXTURE_NODE" "$GENTS_FIXTURE_SCRIPT"\n');
 const controller=new AbortController();t.after(()=>controller.abort());
 const env={...localEnv(),GENTS_NO_OPEN:'1',GENTS_FIXTURE_MODE:mode,GENTS_FIXTURE_STATE:state,GENTS_FIXTURE_PORT:String(port),GENTS_FIXTURE_NODE:process.execPath,GENTS_FIXTURE_SCRIPT:fixture};
 return {root,state,port,controller,env,options:{root,office:`http://127.0.0.1:${port}`,env,signal:controller.signal,shutdownTimeoutMs:150,startupTimeoutMs:1000,restartDelayMs:5000}};
}
test('the actual supervisor cancels before readiness and after an unsolicited exit',async t=>{
 for(const mode of ['never-ready','crash-after-ready']){
  const f=await supervisorFixture(t,mode),started=performance.now(),running=startCompany(f.options);
  if(mode==='never-ready')await until(()=>readFile(f.state+'.waiting'));
  else{await until(async()=>(await readFile(f.state+'.commands','utf8')).includes('status'));await delay(900);}
  f.controller.abort(new Error('fixture stop'));
  assert.equal(await running,0);assert.ok(performance.now()-started<3000);
  assert.equal((await readFile(f.state+'.starts','utf8')).trim(),'start');
 }
});
test('onboarding failure cleans the launched service; restart 75 is retained',async t=>{
 const failed=await supervisorFixture(t,'ready');failed.env.GENTS_FIXTURE_ONBOARD_FAIL='1';
 assert.equal(await startCompany(failed.options),1);
 await assert.rejects(fetch(`http://127.0.0.1:${failed.port}/api/config`));
 const restart=await supervisorFixture(t,'restart-once'),running=startCompany(restart.options);
 await until(async()=>(await readFile(restart.state+'.starts','utf8')).trim().split('\n').length===2);
 await until(async()=>(await readFile(restart.state+'.commands','utf8')).split('status').length===3);
 restart.controller.abort();assert.equal(await running,0);
});
test('cancellation bounds a stubborn setup command and readiness timeout cleans the service',async t=>{
 const setup=await supervisorFixture(t,'ready');setup.env.GENTS_FIXTURE_DOCTOR_WAIT='1';
 const running=startCompany(setup.options);
 await until(()=>readFile(setup.state+'.doctor'));const start=performance.now();setup.controller.abort();
 assert.equal(await running,0);assert.ok(performance.now()-start<1000);
 const timeout=await supervisorFixture(t,'never-ready');timeout.options.startupTimeoutMs=150;
 assert.equal(await startCompany(timeout.options),1);
 assert.equal((await readFile(timeout.state+'.starts','utf8')).trim(),'start');
});
test('an already-running compatible company is reused and never owned or stopped',async t=>{
 const f=await supervisorFixture(t,'ready'),existing=launch('ready',f.state,f.port);t.after(()=>existing.stop());
 await until(()=>readFile(f.state+'.listener'));
 assert.equal(await startCompany(f.options),0);
 assert.equal(existing.result,undefined);
 assert.ok((await fetch(`http://127.0.0.1:${f.port}/api/config`)).ok);
 assert.equal((await readFile(f.state+'.starts','utf8')).trim(),'start');
});
test('an incompatible existing listener is rejected and left running',async t=>{
 const f=await supervisorFixture(t,'ready'),existing=launch('unsealed',f.state,f.port);t.after(()=>existing.stop());
 await until(()=>readFile(f.state+'.listener'));
 assert.equal(await startCompany(f.options),1);
 assert.equal(existing.result,undefined);
 assert.ok((await fetch(`http://127.0.0.1:${f.port}/api/config`)).ok);
 assert.equal((await readFile(f.state+'.starts','utf8')).trim(),'start');
});

test('interactive reuse keeps following until Ctrl+C without taking ownership',async t=>{
 const f=await supervisorFixture(t,'ready'),existing=launch('ready',f.state,f.port);t.after(()=>existing.stop());
 await until(()=>readFile(f.state+'.listener'));
 let finished=false;
 const running=startCompany({...f.options,follow:true}).then(code=>{finished=true;return code;});
 await until(async()=>(await readFile(f.state+'.commands','utf8')).includes('status'));
 await delay(60);assert.equal(finished,false);
 f.controller.abort();assert.equal(await running,0);assert.equal(existing.result,undefined);
 assert.ok((await fetch(`http://127.0.0.1:${f.port}/api/config`)).ok);
});
