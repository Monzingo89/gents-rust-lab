import test from'node:test';import assert from'node:assert/strict';
import{mkdtemp,writeFile,rm}from'node:fs/promises';import{tmpdir}from'node:os';import{join}from'node:path';import{spawnSync}from'node:child_process';
import{enumerationInputs,enumerationCoverage,enumerationRust}from'./fixtures/mcp-weighted-slot-inputs.mjs';
test('actual Rust generator matches audited inputs for every seed and produces 512 distinct sets',async()=>{
 const root=await mkdtemp(join(tmpdir(),'gents-slot-inputs-'));
 try{
  const source=join(root,'inputs.rs'),binary=join(root,process.platform==='win32'?'inputs.exe':'inputs');
  await writeFile(source,enumerationRust+'\nfn main(){for seed in 0..512u64{println!("{:?}",enumeration_requests(seed));}}');
  const build=spawnSync('rustc',['--edition=2021',source,'-o',binary],{encoding:'utf8',timeout:30000});assert.equal(build.status,0,build.stderr);
  const result=spawnSync(binary,[],{encoding:'utf8',timeout:5000,maxBuffer:1024*1024});assert.equal(result.status,0,result.stderr);
  const actual=result.stdout.trim().split('\n').map(line=>JSON.parse(line.replaceAll('(','[').replaceAll(')',']')));
  assert.equal(actual.length,512);
  for(let seed=0;seed<512;seed++)assert.deepEqual(actual[seed],enumerationInputs(seed));
  assert.equal(enumerationCoverage().distinct_inputs,512);
  assert(actual.flat().every(([start,end,weight])=>start>=0&&start<end&&weight>0));
  assert.throws(()=>enumerationInputs(512));
 }finally{await rm(root,{recursive:true,force:true});}
});
