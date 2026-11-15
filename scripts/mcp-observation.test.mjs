import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {join} from 'node:path';
import {saveObservation} from './mcp-observation.mjs';

test('live snapshots preserve terminal evidence and never label runtime completion as independent PASS',async()=>{
 const path=await mkdtemp(join(tmpdir(),'gents-observation-')),folder=pathToFileURL(path+'/');
 try{
  await writeFile(new URL('report.json',folder),'terminal evidence');
  const args={folder,fixture:{title:'Trial',kind:'rust_library'},report:{run:'r',company:'c',status:'PASS'},plan:{messages:[{role:'user',message:'<script>fake user</script>'}]},saved:{run_id:'r',company_id:'c',status:'completed',events:[{seq:1,agent:'Local author',kind:'call_completed',reply:{text:'<script>candidate</script>',usage:{cost_usd:0}}}]},now:123};
  const result=await saveObservation(args);
  assert.equal(result.status,'OBSERVATION_ONLY');assert.equal(result.independently_admitted,false);assert.equal(result.runtime_status,'completed');
  const html=await readFile(new URL('observation.html',folder),'utf8');
  assert(html.includes('OBSERVATION_ONLY'));assert(html.includes('Scripted fake user'));assert(html.includes('#1 Local author'));assert(!html.includes('<script>'));
  assert.equal(await readFile(new URL('report.json',folder),'utf8'),'terminal evidence');
  await assert.rejects(saveObservation({...args,saved:{...args.saved,run_id:'other'}}),/scope mismatch/);
 }finally{await rm(path,{recursive:true,force:true});}
});
