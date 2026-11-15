// Trusted-host continuation of one stopped, reviewed local task. No task authoring,
// permission changes, acceptance waiver or replacement run UUID.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash,randomBytes,randomUUID} from 'node:crypto';
import {lstat,readFile,writeFile} from 'node:fs/promises';
import {parseArgs} from 'node:util';
import {fileURLToPath} from 'node:url';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {evaluationProfile} from './mcp-evaluation-profile.mjs';
import {acceptancePreflight} from './mcp-rust-acceptance.mjs';
import {journal} from './mcp-curriculum-bridge.mjs';
import {validatePlanningContinuation} from './mcp-planning-continuation.mjs';

const root=new URL('../',import.meta.url);
const {values}=parseArgs({options:Object.fromEntries(['report','store','runtime-config','reason'].map(k=>[k,{type:'string'}]))});
const uuid='[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}';
const folderPattern=new RegExp(`^data/mcp-workflow-check/${uuid}$`);
const storePattern=new RegExp(`^data/mcp-workflow-check/${uuid}/runs\\.sqlite3$`);
assert(typeof values.report==='string'&&folderPattern.test(values.report),'--report must identify one private workflow UUID directory');
assert(typeof values.store==='string'&&storePattern.test(values.store),'--store must identify one private workflow runs.sqlite3');
assert(typeof values.reason==='string'&&values.reason.trim().length>0&&values.reason.length<=1000,'--reason is required and bounded');
assert(typeof values['runtime-config']==='string','--runtime-config is required');

async function ordinary(path,maxBytes){
 const url=new URL(path,root);
 const info=await lstat(url);
 assert(info.isFile()&&!info.isSymbolicLink()&&info.nlink===1&&info.size<=maxBytes,'Continuation evidence must be a bounded ordinary file');
 for(let current=new URL('.',url);current.href!==root.href;current=new URL('../',current))assert(!(await lstat(current)).isSymbolicLink(),'Symlink in continuation evidence path');
 return url;
}
async function json(path){return JSON.parse(await readFile(await ordinary(path,8*1024*1024),'utf8'));}
function policyView(policy){return {...policy,...Object.fromEntries(['grants','ceilings'].map(key=>[key,Object.fromEntries(Object.entries(policy[key]).map(([actor,grant])=>[actor,Object.fromEntries(Object.entries(grant).map(([name,values])=>[name,[...values].sort()]))]))]))};}
async function command(script,args=[]){
 const child=spawn(process.execPath,[fileURLToPath(new URL(script,root)),...args],{cwd:root,stdio:'inherit'});
 return await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',code=>resolve(code));});
}

const prior=await json(values.report+'/report.json');
const planningOnly=prior.run===null;
assert(planningOnly?['FAIL','OBSERVATION_ERROR_LIVE_WORK_RETAINED'].includes(prior.status):prior.status==='FAIL','Only failed/interrupted evidence may continue');
assert.equal(prior.company,'mcp-workflow-check');
assert(new RegExp(`^${uuid}$`).test(planningOnly?prior.planning_session:prior.run),'Canonical prior identity required');
const fixture=fixtureFromId(prior.fixture_id);
assert.equal(prior.fixture_sha,fixtureDigest(fixture),'Cannot continue an older fixture as a newer suite');
acceptancePreflight(root,fixture); // Reject host fixture defects before changing any lease.
await ordinary(values.store,2*1024*1024*1024);
const profile=await evaluationProfile(root,values['runtime-config']);
assert(!(await fetch('http://127.0.0.1:8796/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null)),'Existing benchmark listener retained; no live task stopped');
if(planningOnly){
 assert.notEqual(fixture.kind,'rust_library','Planning continuation currently supports native artifact workflows');
 const {planning:checkpoint}=await journal(root,{op:'planning_authority',company:prior.company,session:prior.planning_session});
 validatePlanningContinuation(prior,fixture,profile,checkpoint);
 const code=await command('scripts/mcp-workflow-check.mjs',['--preflight-planning','--case',fixture.id,'--planning-session',prior.planning_session,'--planning-origin-report',values.report,'--existing-store',values.store,'--runtime-config',values['runtime-config'],'--resume-reason',values.reason]);
 assert.equal(code,0,'Planning runner preflight failed before lease mutation');
}else{
const review=await json(values.report+'/execution-review.json');
const authority=await journal(root,{op:'native_authority',company:prior.company,run:prior.run});
assert.equal(authority.digest,review.digest);
assert.equal(canonical(authority.intake),canonical(review.intake));
assert.equal(canonical(policyView(authority.policy)),canonical(policyView(review.policy)));
}

let status=await journal(root,{op:'status'});
if(status.active){
 const active=status.active;
 assert.equal(active.id,values.report.split('/').at(-1),'Unrelated active lease retained');
 assert.equal(active.case_id,fixture.id);
 assert.equal(active.fixture_sha,fixtureDigest(fixture));
 const savedLease=await json('data/mcp-development-curriculum/leases/'+active.id+'.json');
 status=await journal(root,{op:'finish',attempt:active.id,lease:savedLease.lease,state:'fail',proof:{fixture_sha:fixtureDigest(fixture),reason:'Recorded runtime/profile maintenance continuation: '+values.reason,report:values.report+'/report.json',run:prior.run}});
}
assert.equal(status.active,null);
const attempt=randomUUID(),lease=randomBytes(32).toString('hex');
await writeFile(new URL(`data/mcp-development-curriculum/leases/${attempt}.json`,root),JSON.stringify({attempt,case_id:fixture.id,fixture_sha:fixtureDigest(fixture),lease,run:prior.run,planning_session:prior.planning_session,continuation_of:values.report,maintenance_reason:values.reason}),{flag:'wx',mode:0o600});
await journal(root,{op:'begin',attempt,case_id:fixture.id,fixture_sha:fixtureDigest(fixture),origin:'executed',lease_sha:createHash('sha256').update(lease).digest('hex')});
const folder=`data/mcp-workflow-check/${attempt}`;
console.log(JSON.stringify({status:'CONTINUATION_LEASED',attempt,run:prior.run,prior_report:values.report}));
const code=await command(fixture.kind==='rust_library'?'scripts/mcp-coding-check.mjs':'scripts/mcp-workflow-check.mjs',['--case',fixture.id,'--attempt-id',attempt,...(planningOnly?['--planning-session',prior.planning_session,'--planning-origin-report',values.report,...(prior.owner_authorization_mode==='approved_workflow'?['--approved-workflow']:[])]:['--resume-report',values.report]),'--existing-store',values.store,'--runtime-config',values['runtime-config'],'--resume-reason',values.reason]);
const report=await json(folder+'/report.json');
if(code===0&&report.status==='PASS'){
 // Reuse the independent canonical verifier and active-lease reconciliation.
 const accepted=await command('scripts/mcp-development-curriculum.mjs',['--reconcile',folder]);
 assert.equal(accepted,0,'Canonical admission failed; active lease retained for inspection');
}else if(report.status==='FAIL'){
 await journal(root,{op:'finish',attempt,lease,state:'fail',proof:{fixture_sha:fixtureDigest(fixture),reason:report.error??'Continuation failed',report:folder+'/report.json',run:prior.run}});
 process.exitCode=1;
}else{
 throw Error('Live or uncertain continuation retained; inspect the same run, never start a replacement');
}
