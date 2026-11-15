import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
test('invalid curriculum/workflow options fail before starting a listener or task',()=>{
 for(const [script,args] of [['mcp-development-curriculum.mjs',['--status','--approved-workflow']],['mcp-workflow-check.mjs',['--case']],['mcp-workflow-check.mjs',['--unknown']],['mcp-development-curriculum.mjs',['--next','--status']],['mcp-development-curriculum.mjs',['--status','--runtime-config','config/profile.json']],['mcp-development-curriculum.mjs',['--adopt']],['mcp-development-curriculum.mjs',['--retest']],['mcp-development-curriculum.mjs',['--retest','depot-reconciliation','--next']]]){
  const result=spawnSync(process.execPath,[fileURLToPath(new URL(script,import.meta.url)),...args],{encoding:'utf8',timeout:10000});assert.notEqual(result.status,0);assert(!result.stdout.includes('STARTED'));assert.match(result.stderr,/argument missing|Unknown option|exactly one|requires --next or --retest/i);
 }
});
