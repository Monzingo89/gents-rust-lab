// Independent, model-free checks against exact frozen owner configuration.
import assert from 'node:assert/strict';
import {readFile,lstat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {relative,resolve,sep} from 'node:path';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
const sha=value=>createHash('sha256').update(value).digest('hex');
export function planningPolicyDigest(policy){
 const normalized={...policy,candidates:policy.candidates.map(candidate=>({...candidate,model:{url:null,key_env:null,path:null,adapter:null,...candidate.model}}))};
 return sha(canonical(normalized));
}
export async function frozenPlanningPolicy(root,profile){
 if(!profile?.frozen_path)return null;
 const base=fileURLToPath(root),file=resolve(profile.frozen_path),name=relative(base,file).split(sep).join('/');
 assert(/^data\/mcp-workflow-check\/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}\/runtime-profile\.json$/.test(name),'Frozen profile is outside private workflow evidence');
 const url=pathToFileURL(file),info=await lstat(url);assert(info.isFile()&&!info.isSymbolicLink()&&info.nlink===1&&info.size<=65536,'Unsafe frozen planning profile');
 for(let current=new URL('.',url);current.href!==root.href;current=new URL('../',current))assert(!(await lstat(current)).isSymbolicLink(),'Symlink frozen planning profile');
 const bytes=await readFile(url);assert.equal(sha(bytes),profile.sha256,'Frozen runtime profile digest changed');
 return JSON.parse(bytes.toString('utf8')).planning_model_policy??null;
}
export function verifyPlanningPolicy(attempts,policy,{boundaries=[],accepted=null}={}){
 if(!policy){assert(attempts.every(a=>!a.planning_model_selection),'Canonical model selection has no frozen owner policy');return {planning_model_selection_mode:'fixed_admin_model',planning_model_switches:0};}
 assert(Array.isArray(boundaries)&&boundaries.length<=1,'Ambiguous native phase boundary');
 const boundary=boundaries[0];
 if(boundary){
  assert(accepted,'Phase boundary requires frozen owner adoption');
  assert.deepEqual(boundary,{kind:'owned_selection_adopted',start_revision:accepted.owner_confirmed_revision,proposal_sha256:accepted.proposal_sha256,data_sha256:accepted.data_sha256,catalog_sha256:accepted.catalog_sha256});
  assert(Number.isSafeInteger(boundary.start_revision)&&boundary.start_revision>=2&&boundary.start_revision<=9);
  for(const key of ['proposal_sha256','data_sha256','catalog_sha256'])assert(/^[a-f0-9]{64}$/.test(boundary[key]));
  assert(attempts.some(a=>a.revision===boundary.start_revision-1&&a.status==='selection_review'),'Owner adoption did not follow a completed review');
 }
 const digest=planningPolicyDigest(policy),used=[];let failures=0,skips=0,reset=false,lastRevision=0;

 assert(policy.candidates.length>=1&&policy.candidates.length<=4&&Number.isSafeInteger(policy.failures_before_next)&&policy.failures_before_next>=1&&policy.failures_before_next<=3);
 for(const attempt of attempts){
  assert(Number.isSafeInteger(attempt.revision)&&attempt.revision>lastRevision,'Unordered planning revisions');lastRevision=attempt.revision;
  const phase=boundary&&attempt.revision>=boundary.start_revision?boundary:{kind:'session',start_revision:1};
  if(phase===boundary&&!reset){failures=0;reset=true;}
  const proof=attempt.planning_model_selection;assert(proof,'Missing canonical planning-model selection');
  if(proof.phase_scope)assert.deepEqual(proof.phase_scope,phase,'Canonical phase differs from native owner adoption');
  else assert(!reset,'Phase reset has no native selection proof');
  assert.equal(proof.policy_sha256,digest,'Canonical selection differs from frozen owner policy');
  assert.equal(proof.structural_failures,failures);
  const requested=Math.min(Math.floor(failures/policy.failures_before_next),policy.candidates.length-1);assert.equal(proof.requested_index,requested);
  assert(Number.isSafeInteger(proof.candidate_index)&&proof.candidate_index>=requested&&proof.candidate_index<policy.candidates.length);
  const candidate=policy.candidates[proof.candidate_index];assert.equal(proof.candidate_id,candidate.id);assert.equal(proof.weight_sha256,candidate.sha256);assert.equal(candidate.model.provider,'local');assert.equal(attempt.model,candidate.model.model);
  assert(Number.isSafeInteger(proof.weight_bytes)&&proof.weight_bytes>0&&proof.weight_bytes<=128*1024**3);
  assert(Number.isSafeInteger(proof.host_memory?.effective_limit_bytes)&&proof.host_memory.effective_limit_bytes>=candidate.minimum_memory_mib*1024**2,'Selected model exceeds owner capacity floor');
  assert.equal(proof.skipped.length,proof.candidate_index-requested);
  const reasons=new Set(['host_capacity_below_owner_floor','model_not_installed_or_unreadable','model_unreadable','model_metadata_unavailable','model_root_unavailable','model_root_rejected','model_path_missing','model_symlink_rejected','model_owner_or_directory_permissions_rejected','model_not_bounded_regular_file','model_links_or_permissions_rejected','model_read_failed','model_changed_during_inspection','model_hash_pin_mismatch']);
  proof.skipped.forEach((entry,index)=>{assert.equal(entry.candidate_id,policy.candidates[requested+index].id);assert(reasons.has(entry.reason));});skips+=proof.skipped.length;
  const native=attempt.local_execution;assert(native&&native.backend_pid>0);assert.equal(native.observed_context_tokens,candidate.model.context_tokens??16384);
  assert.equal(native.generation_budget.reasoning_tokens,candidate.model.local_reasoning_tokens??0);
  assert.equal(native.generation_budget.final_tokens_reserved,candidate.model.local_final_tokens??attempt.reply_token_limit);
  used.push(candidate.id);
  if(attempt.status==='failed'&&typeof attempt.validation_error==='string'&&native.finish_reason==='stop')failures++;
 }
 return {planning_model_selection_mode:'owner_local_policy',planning_model_policy_sha256:digest,planning_model_candidates_used:[...new Set(used)],planning_model_switches:used.filter((id,index)=>index>0&&id!==used[index-1]).length,planning_model_candidate_skips:skips};
}
