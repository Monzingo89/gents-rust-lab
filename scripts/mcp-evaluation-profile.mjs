// Trusted-host evaluation configuration; model proposals cannot select this.
import {readFile,lstat,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
export async function evaluationProfile(root,path='config/company.mcp-development.json'){
 assert(/^config\/[a-zA-Z0-9._-]+\.json$/.test(path),'Evaluation profile must be a local config JSON file');
 const folder=await lstat(new URL('config/',root)),url=new URL(path,root),info=await lstat(url);
 assert(!folder.isSymbolicLink()&&info.isFile()&&!info.isSymbolicLink()&&info.nlink===1&&info.size<=65536,'Unsafe evaluation configuration');
 const bytes=await readFile(url),config=JSON.parse(bytes.toString('utf8'));
 assert(config.company_mode===true&&config.company_tools===true&&Array.isArray(config.agents)&&config.agents.length>0,'Company evaluation configuration required');
 const policy=config.planning_model_policy;
 assert(policy===undefined||policy===null||(typeof policy==='object'&&!Array.isArray(policy)),'Planning policy must be an owner configuration object');
 if(policy){
  assert.deepEqual(Object.keys(policy).sort(),['candidates','failures_before_next'],'Unknown planning policy field');
  assert(Number.isInteger(policy.failures_before_next)&&policy.failures_before_next>=1&&policy.failures_before_next<=3,'Invalid planning escalation threshold');
  assert(Array.isArray(policy.candidates)&&policy.candidates.length>=1&&policy.candidates.length<=4,'Planning policy needs 1..4 candidates');
  const ids=new Set();
  for(const candidate of policy.candidates){
   assert.deepEqual(Object.keys(candidate).sort(),['id','minimum_memory_mib','model','sha256'],'Unknown planning candidate field');
   assert(typeof candidate.id==='string'&&/^[a-z0-9_-]{1,64}$/.test(candidate.id)&&!ids.has(candidate.id),'Invalid or duplicate planning candidate id');ids.add(candidate.id);
   assert(typeof candidate.sha256==='string'&&/^[a-f0-9]{64}$/.test(candidate.sha256),'Exact planning model SHA-256 pin required');
   assert(Number.isSafeInteger(candidate.minimum_memory_mib)&&candidate.minimum_memory_mib>=512&&candidate.minimum_memory_mib<=2*1024*1024,'Invalid planning capacity floor');
  }
 }
 const models=[config.admin_judge,...config.agents.flatMap(a=>[a.planner,a.worker]),...(policy?.candidates??[]).map(c=>c.model)];
 assert(models.every(m=>m?.provider==='local'&&typeof m.path==='string'&&/^models\/[a-zA-Z0-9._/-]+\.gguf$/.test(m.path)&&!m.path.split('/').includes('..')),'Evaluation models must use local GGUF paths; no remote providers');
 assert(models.every(m=>m.local_reasoning_tokens===undefined||(Number.isInteger(m.local_reasoning_tokens)&&m.local_reasoning_tokens>=0&&m.local_reasoning_tokens<=4096)),'Local reasoning budgets must be integers in 0..4096');
 assert(models.every(m=>m.local_final_tokens===undefined||(Number.isInteger(m.local_final_tokens)&&m.local_final_tokens>=1&&m.local_final_tokens<=4096)),'Local final budgets must be integers in 1..4096');
 assert(models.every(m=>m.context_tokens===undefined||(Number.isInteger(m.context_tokens)&&m.context_tokens>=2048&&m.context_tokens<=16384)),'Local context budgets must be integers in 2048..16384');
 return {path,sha256:createHash('sha256').update(bytes).digest('hex'),models:models.map(m=>({provider:m.provider,model:m.model,path:m.path,context_tokens:m.context_tokens,local_reasoning_tokens:m.local_reasoning_tokens,local_final_tokens:m.local_final_tokens})),...(policy?{planning_model_policy:policy}:{})};
}
export async function freezeEvaluationProfile(root,folder,profile,prior=null,resumeReason=null){
 const changed=Boolean(prior)&&(profile.path!==(prior.runtime_profile?.path??'config/company.mcp-development.json')||(prior.runtime_profile?.sha256!==undefined&&profile.sha256!==prior.runtime_profile.sha256));
 if(changed)assert(typeof resumeReason==='string'&&resumeReason.trim().length>0&&resumeReason.length<=1000,'A model-profile intervention requires a recorded resume reason');
 const bytes=await readFile(new URL(profile.path,root));assert.equal(createHash('sha256').update(bytes).digest('hex'),profile.sha256,'Profile changed before freezing');
 const frozen=new URL('runtime-profile.json',folder);await writeFile(frozen,bytes,{flag:'wx',mode:0o600});
 return {profile:{...profile,frozen_path:fileURLToPath(frozen)},changed};
}
