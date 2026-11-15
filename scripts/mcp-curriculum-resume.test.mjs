import test from 'node:test';import assert from 'node:assert/strict';import{spawnSync}from'node:child_process';
test('continuation rejects unsafe or incomplete input before journal mutation or inference',()=>{
 const id='00000000-0000-0000-0000-000000000000';
 const valid=['--report',`data/mcp-workflow-check/${id}`,'--store',`data/mcp-workflow-check/${id}/runs.sqlite3`,'--runtime-config','config/company.mcp-development.json','--reason','Fixture validation only'];
 for(const args of [[],['--unknown'],valid.map((v,i)=>i===1?'../outside':v),valid.map((v,i)=>i===3?'../outside.sqlite3':v),valid.map((v,i)=>i===7?'':v),valid]){
  const result=spawnSync(process.execPath,['scripts/mcp-curriculum-resume.mjs',...args],{encoding:'utf8',timeout:10000});
  assert.notEqual(result.status,0);assert(!result.stdout.includes('CONTINUATION_LEASED'));
  if(args===valid)assert(result.stderr.includes('ENOENT'),'Valid path syntax must reach the missing-file check');
 }
});
