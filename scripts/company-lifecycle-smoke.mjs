// Real Rust control plane + sealed bundle in a disposable private workspace.
// No production config, company stores, model weights or credentials are used.
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,copyFile,writeFile,chmod,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {randomUUID} from 'node:crypto';
import {createServer} from 'node:net';
import {execFileSync} from 'node:child_process';
import {nativeRoles,createRuntimeBundle,bundleEnvironment,bundleExecutable,verifyBundleBinding} from './mcp-runtime-bundle.mjs';
import {startOwnedProcess,waitForReadiness,probeCompany} from './company-lifecycle.mjs';

const source=new URL('../',import.meta.url),folder=await mkdtemp(join(tmpdir(),'gents-real-lifecycle-'));
const root=pathToFileURL(folder+'/');
const listener=createServer();await new Promise(resolve=>listener.listen(0,'127.0.0.1',resolve));
const port=listener.address().port;await new Promise(resolve=>listener.close(resolve));
const origin=`http://127.0.0.1:${port}`,startupId=randomUUID();
let child,bundle;
try{
 await mkdir(join(folder,'target/debug'),{recursive:true});
 for(const role of nativeRoles)await copyFile(new URL('target/debug/'+role,source),join(folder,'target/debug',role));
 bundle=await createRuntimeBundle(root);
 const model={provider:'mock',model:'lifecycle-only-no-dispatch'};
 await writeFile(join(folder,'config.json'),JSON.stringify({company_mode:true,company_tools:true,admin_judge:model,agents:[{id:'Fixture',assignment:'No task execution',planner:model,worker:model}],max_steps:1,max_tokens:128,max_run_seconds:5}));
 const env=Object.fromEntries(['PATH','HOME','TMPDIR','LANG'].filter(k=>k in process.env).map(k=>[k,process.env[k]]));
 Object.assign(env,bundleEnvironment(bundle),{AUTH_MODE:'local',BIND:`127.0.0.1:${port}`,CONFIG:'config.json',RUN_STORE:'runs.sqlite3',GENTS_STARTUP_ID:startupId,GENTS_LOCAL_ONLY:'1',GENTS_MCP_TRANSPORT:'stdio',GENTS_OFFICE_ACTIVITY:'0',GENTS_DISABLE_SCHEDULER:'1'});
 const start=performance.now();
 child=startOwnedProcess(bundleExecutable(root,bundle,'gents-harness'),[],{cwd:folder,env,stdio:'ignore'});
 const config=await waitForReadiness({owned:child,probe:signal=>probeCompany({origin,startupId,signal}),timeoutMs:15000,pollMs:25});
 const readyMs=performance.now()-start;
 assert.equal(config.startup_id,startupId);
 verifyBundleBinding(await(await fetch(origin+'/api/mcp/catalog')).json(),bundle);
 await assert.rejects(probeCompany({origin,startupId:'wrong-instance'}),/not the process instance/);
 assert.deepEqual((await(await fetch(origin+'/api/runs')).json()).runs,[]);
 let residentBytes=null;
 try{residentBytes=Number(execFileSync('ps',['-o','rss=','-p',String(child.child.pid)],{encoding:'utf8',timeout:1000}).trim())*1024;}catch{}
 const stopping=performance.now(),result=await child.stop('SIGTERM',3000),shutdownMs=performance.now()-stopping;
 assert.equal(result.code,0);assert.equal(result.forced,false);
 await assert.rejects(fetch(origin+'/api/config'));
 console.log(JSON.stringify({fixture:'actual-rust-lifecycle',passed:true,instance_binding:true,sealed_bundle_verified:true,graceful_shutdown:true,listener_closed:true,
  harness_sha256:bundle.executables['gents-harness'].sha256,
  measurements:{startup_ms:Math.round(readyMs*1000)/1000,shutdown_ms:Math.round(shutdownMs*1000)/1000,resident_bytes:residentBytes,resident_scope:'single idle harness RSS sample, not model/peak/GPU memory',generative_model_calls:0,paid_calls:0,actual_provider_cost_usd:0,verified_real_tasks:null,model_context_tokens:null,estimated_total_cost_usd:null}}));
}finally{
 await child?.stop('SIGTERM',3000);
 if(bundle)await chmod(new URL(bundle.manifest.replace(/manifest\.json$/,''),root),0o700);
 await rm(folder,{recursive:true,force:true});
}
