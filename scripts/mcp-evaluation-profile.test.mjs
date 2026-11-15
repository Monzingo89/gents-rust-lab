import test from 'node:test';import assert from 'node:assert/strict';import {mkdtemp,mkdir,writeFile,symlink,rm} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';import {pathToFileURL} from 'node:url';
import {evaluationProfile,freezeEvaluationProfile} from './mcp-evaluation-profile.mjs';
test('only bounded ordinary local model profiles can enter the evaluation runner',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'gents-profile-')),root=pathToFileURL(dir+'/');
 try{
  await mkdir(new URL('config/',root));const model={provider:'local',model:'fixture',path:'models/fixture/model.gguf',context_tokens:8192};
  const config={company_mode:true,company_tools:true,admin_judge:model,agents:[{id:'Author',planner:model,worker:model}]};
  const file=new URL('config/profile.json',root);await writeFile(file,JSON.stringify(config));const original=await evaluationProfile(root,'config/profile.json');assert.equal(original.sha256.length,64);
  assert.equal(original.models[0].local_reasoning_tokens,undefined);
  await writeFile(file,JSON.stringify({...config,admin_judge:{...model,local_reasoning_tokens:2048}}));
  const bounded=await evaluationProfile(root,'config/profile.json');assert.equal(bounded.models[0].local_reasoning_tokens,2048);assert.notEqual(bounded.sha256,original.sha256);
  await writeFile(file,JSON.stringify({...config,admin_judge:{...model,local_final_tokens:2048}}));
  const output=await evaluationProfile(root,'config/profile.json');assert.equal(output.models[0].local_final_tokens,2048);assert.notEqual(output.sha256,original.sha256);
  for(const cap of [0,-1,1.5,4097,'2048',null]){await writeFile(file,JSON.stringify({...config,admin_judge:{...model,local_final_tokens:cap}}));await assert.rejects(evaluationProfile(root,'config/profile.json'));}
  for(const cap of [-1,1.5,4097,'512']){await writeFile(file,JSON.stringify({...config,admin_judge:{...model,local_reasoning_tokens:cap}}));await assert.rejects(evaluationProfile(root,'config/profile.json'));}
  await assert.rejects(evaluationProfile(root,'config/../profile.json'));
  await writeFile(file,JSON.stringify({...config,admin_judge:{...model,provider:'openai'}}));await assert.rejects(evaluationProfile(root,'config/profile.json'));
  await writeFile(file,JSON.stringify({...config,admin_judge:{...model,path:'models/../private.gguf'}}));await assert.rejects(evaluationProfile(root,'config/profile.json'));
  await symlink(file,new URL('config/linked.json',root));await assert.rejects(evaluationProfile(root,'config/linked.json'));
 }finally{await rm(dir,{recursive:true,force:true});}
});

test('freezing uses exact reviewed bytes, requires intervention reasons and never overwrites a prior snapshot',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'gents-profile-freeze-')),root=pathToFileURL(dir+'/');
 try{
  await mkdir(new URL('config/',root));await mkdir(new URL('attempt/',root));
  const model={provider:'local',path:'models/fixture/model.gguf',context_tokens:16384},config={company_mode:true,company_tools:true,admin_judge:model,agents:[{id:'Writer',planner:model,worker:model}]};
  const file=new URL('config/wide.json',root);await writeFile(file,JSON.stringify(config));const selected=await evaluationProfile(root,'config/wide.json');
  await assert.rejects(freezeEvaluationProfile(root,new URL('attempt/',root),selected,{runtime_profile:{path:'config/old.json',sha256:'0'.repeat(64)}}),/recorded resume reason/);
  const frozen=await freezeEvaluationProfile(root,new URL('attempt/',root),selected,{runtime_profile:{path:'config/old.json',sha256:'0'.repeat(64)}},'Measured required prompt exceeds prior host context');assert.equal(frozen.changed,true);assert(frozen.profile.frozen_path.endsWith('/attempt/runtime-profile.json'));
  await assert.rejects(freezeEvaluationProfile(root,new URL('attempt/',root),selected),/EEXIST/);
  await mkdir(new URL('changed/',root));await writeFile(file,JSON.stringify({...config,max_tokens:512}));await assert.rejects(freezeEvaluationProfile(root,new URL('changed/',root),selected),/changed before freezing/);
  for(const context of [0,2047,16385,8192.5,'8192']){await writeFile(file,JSON.stringify({...config,admin_judge:{...model,context_tokens:context}}));await assert.rejects(evaluationProfile(root,'config/wide.json'),/context budgets/);}
 }finally{await rm(dir,{recursive:true,force:true});}
});

test('all planning alternatives are frozen and validated as local bounded pinned models',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'gents-planning-profile-')),root=pathToFileURL(dir+'/');
 try{
  await mkdir(new URL('config/',root));const model={provider:'local',model:'fixture',path:'models/fixture/model.gguf',context_tokens:8192};
  const policy={failures_before_next:2,candidates:[{id:'reasoner',model:{...model,local_reasoning_tokens:512},sha256:'a'.repeat(64),minimum_memory_mib:8192}]};
  const config={company_mode:true,company_tools:true,admin_judge:model,agents:[{id:'Author',planner:model,worker:model}],planning_model_policy:policy},file=new URL('config/profile.json',root);
  await writeFile(file,JSON.stringify(config));const selected=await evaluationProfile(root,'config/profile.json');assert.equal(selected.models.length,4);assert.deepEqual(selected.planning_model_policy,policy);
  for(const invalid of [false,{...policy,failures_before_next:0},{...policy,unknown:true},{...policy,candidates:[]},{...policy,candidates:[{...policy.candidates[0],sha256:'bad'}]},{...policy,candidates:[{...policy.candidates[0],model:{...model,provider:'openai'}}]}]){
   await writeFile(file,JSON.stringify({...config,planning_model_policy:invalid}));await assert.rejects(evaluationProfile(root,'config/profile.json'));
  }
 }finally{await rm(dir,{recursive:true,force:true});}
});
