import test from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';import {spawnSync} from 'node:child_process';
import {balancedInputs,coverage,oracleRust} from './fixtures/mcp-settlement-inputs.mjs';
import {fixtureFromId,fixtureDigest,levels} from './fixtures/mcp-workflow-catalog.mjs';
import {reviewCodingPlan} from './mcp-coding-verifier.mjs';

// Independent small-instance oracle enumerates transfer choices, rather than
// the Rust acceptance oracle's zero-sum partitions. It returns a count only.
const memo=new Map();
function transferCount(input){
 const values=input.filter(v=>v!==0).sort((a,b)=>a-b),key=values.join(',');
 if(values.length===0)return 0;if(memo.has(key))return memo.get(key);
 let best=values.length-1;
 for(let from=0;from<values.length;from++)for(let to=0;to<values.length;to++)if(values[from]<0&&values[to]>0){
  const amount=Math.min(-values[from],values[to]),next=[...values];next[from]+=amount;next[to]-=amount;
  best=Math.min(best,1+transferCount(next));
 }
 memo.set(key,best);return best;
}
test('actual Rust corpus and partition oracle match independent bounded transfer enumeration',async()=>{
 const root=await mkdtemp(join(tmpdir(),'gents-settlement-oracle-'));
 try{
  const source=join(root,'oracle.rs'),binary=join(root,process.platform==='win32'?'oracle.exe':'oracle');
  await writeFile(source,oracleRust+'\nfn main(){for code in 0..625usize{let mut rest=code;let mut values=Vec::new();for _ in 0..4{values.push((rest%5) as i64-2);rest/=5;}let sum:i64=values.iter().sum();println!("{:?}|{}",values,if sum==0{optimum_count(&values) as i64}else{-1});}for seed in 0..512{let values=balanced_inputs(seed);println!("{:?}|{}",values,optimum_count(&values));}assert_eq!(optimum_count(&[-5,-4,4,5]),2);assert_eq!(optimum_count(&[i64::MIN,i64::MIN,i64::MAX,i64::MAX,2]),4);assert_eq!(optimum_count(&[-1,-2,-3,-4,-5,-6,6,5,4,3,2,1]),6);assert_eq!(optimum_count(&[]),0);}') ;
  const build=spawnSync('rustc',['--edition=2021',source,'-o',binary],{encoding:'utf8',timeout:30000});assert.equal(build.status,0,build.stderr);
  // Type-check the entire suite with a non-solution stub in this disposable
  // directory only. Never run it or supply it to the company as authored work.
  const contract=join(root,'contract.rs');
  await writeFile(contract,'mod candidate {pub fn settle_balances(_: &[i64])->Option<Vec<(usize,usize,u64)>>{None}}\n'+fixtureFromId('rust-minimum-settlement').testsText);
  const compiled=spawnSync('rustc',['--edition=2021','--test','--emit=metadata',contract,'-o',join(root,'contract.rmeta')],{encoding:'utf8',timeout:30000});assert.equal(compiled.status,0,compiled.stderr);
  const result=spawnSync(binary,[],{encoding:'utf8',timeout:5000,maxBuffer:1024*1024});assert.equal(result.status,0,result.stderr);
  const rows=result.stdout.trim().split('\n').map(line=>{const [values,count]=line.split('|');return {values:JSON.parse(values),count:Number(count)};});
  assert.equal(rows.length,1137);assert.equal(new Set(rows.map(row=>JSON.stringify(row.values))).size,1137);
  let balancedGrid=0;
  for(let i=0;i<rows.length;i++){
   const {values,count}=rows[i];if(i>=625)assert.deepEqual(values,balancedInputs(i-625));
   const balanced=values.reduce((a,b)=>a+b,0)===0;if(i<625&&balanced)balancedGrid++;
   assert.equal(count,balanced?transferCount(values):-1,JSON.stringify(values));
  }
  assert.equal(balancedGrid,85);assert.equal(coverage().distinct_arrays,rows.length);assert.throws(()=>balancedInputs(512));
 }finally{await rm(root,{recursive:true,force:true});}
});
test('ninth scope pins a new optimum problem without implementation, authority or tie requirements',()=>{
 const fixture=fixtureFromId('rust-minimum-settlement');assert.equal(levels.find(l=>l.id===fixture.id).requires[0],'rust-weighted-slots');
 assert.equal(fixture.testsCount,8);assert(Buffer.byteLength(fixture.testsText)<8192);assert(!fixture.testsText.includes('pub fn settle_balances'));
 assert(fixture.requirements.includes('Any globally minimum plan'));assert(!oracleRust.includes('candidate::'));
 const plan={id:'fixture',planning_mode:'implementation',revision:2,status:'ready',messages:[{role:'user',message:fixture.initial},{role:'user',message:fixture.requirements}],proposal:{status:'ready',question:'',assignments:[{actor:'Worker',task:'Author and verify',tools:['files:write','files:read','rust_verify'],acceptance:['Current source passes frozen suite']}],definition_of_done:['Current source passes frozen suite'],native_contracts:{sources:[],artifacts:[]}}};
 const {draft}=reviewCodingPlan(plan,fixture,['Worker']);assert.equal(draft.intake.required_rust_checks[0].source_path,'minimum_settlement.rs');assert.equal(draft.intake.required_rust_checks[0].tests_sha256.length,64);
 assert.notEqual(fixtureDigest(fixture),fixtureDigest({...fixture,testsText:fixture.testsText+'\n'}));
});
