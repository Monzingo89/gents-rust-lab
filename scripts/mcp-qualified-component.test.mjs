import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,chmod,rm,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {exportQualifiedComponent} from './mcp-qualified-component.mjs';
import {fixtureFromId,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
const id='00000000-0000-4000-8000-000000000001',relative='data/mcp-workflow-check/'+id;
test('rejects failed, malformed, unreviewed or non-Rust reports before native review or package publication',async t=>{
 const path=await mkdtemp(join(tmpdir(),'gents-component-reject-')),root=pathToFileURL(path+'/');await chmod(root,0o700);t.after(()=>rm(root,{recursive:true,force:true}));
 await mkdir(new URL(relative+'/',root),{recursive:true,mode:0o700});
 for(const name of ['planning.json','events.json','execution-review.json'])await writeFile(new URL(relative+'/'+name,root),'{}',{mode:0o600});
 const fixture=fixtureFromId('rust-range-union'),valid={status:'PASS',run:id,company:'local-test',fixture_id:fixture.id,fixture_sha:fixtureDigest(fixture)};
 for(const report of [{...valid,status:'FAIL'},{...valid,status:'EXECUTING'},{...valid,run:'../escape'},{...valid,company:'../escape'},
  {...valid,fixture_id:'not-reviewed'},{...valid,fixture_sha:'b'.repeat(64)},{...valid,fixture_id:'reconciliation',fixture_sha:fixtureDigest(fixtureFromId('reconciliation'))}]){
  await writeFile(new URL(relative+'/report.json',root),JSON.stringify(report),{mode:0o600});await assert.rejects(()=>exportQualifiedComponent(root,relative));
 }
 for(const directory of ['../escape',relative+'/',relative+'%2f','https://example.invalid'])await assert.rejects(()=>exportQualifiedComponent(root,directory));
 assert.deepEqual(await readdir(new URL('data/',root)),['mcp-workflow-check']);
});

test('CLI rejects missing, conflicting and unknown options before any export or native process',()=>{
 const file=fileURLToPath(new URL('./mcp-qualified-component.mjs',import.meta.url));
 for(const args of [[],['--report',relative,'--verify','a'.repeat(64)],['--activate'],['--report','../escape'],['--verify','../escape']]){
  const result=spawnSync(process.execPath,[file,...args],{encoding:'utf8',timeout:5000,maxBuffer:16384});
  assert.equal(result.status,1);assert.equal(result.stdout,'');
 }
});
