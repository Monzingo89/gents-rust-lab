import test from 'node:test';
import assert from 'node:assert/strict';
import {evaluationHost} from './mcp-evaluation-host.mjs';
const probe=()=>({platform:()=> 'linux',arch:()=> 'x64',release:()=> 'example-kernel',cpus:()=> [{model:'CPU family'},{model:'CPU family'}],totalmem:()=> 8*1024**3,availableParallelism:()=> 1});
test('records portable observations without equating host memory or CPUs with process/GPU fit',()=>{
 const p=probe();p.hostname=()=>{throw Error('Must not inspect host identity');};
 const value=evaluationHost(p);
 assert.equal(value.logical_cpu_count,2);assert.equal(value.process_available_parallelism,1);
 assert.equal(value.os_reported_memory_bytes,8*1024**3);assert.equal(value.process_arch,'x64');
 assert(!('gpu_memory_bytes' in value));assert(!('hostname' in value));
 assert(value.scope.includes('not a cgroup/process/GPU allowance'));
});
test('unavailable or invalid probes remain unknown rather than invented capacity',()=>{
 const value=evaluationHost(new Proxy({}, {get(){return ()=>{throw Error('Unavailable');};}}));
 for(const key of ['process_platform','process_arch','os_release','first_logical_cpu_model','logical_cpu_count','process_available_parallelism','os_reported_memory_bytes'])assert.equal(value[key],null);
 for(const invalid of [0,-1,Infinity,NaN,1.5,null,undefined,false,Number.MAX_SAFE_INTEGER+1]){
  const p=probe();p.totalmem=()=>invalid;p.availableParallelism=()=>invalid;p.cpus=()=>[];
  const r=evaluationHost(p);assert.equal(r.os_reported_memory_bytes,null);assert.equal(r.process_available_parallelism,null);assert.equal(r.logical_cpu_count,null);
 }
 const p=probe();p.platform=()=> 'bad\nlabel';p.arch=()=> 'x'.repeat(129);p.cpus=()=>[{model:null}];
 const r=evaluationHost(p);assert.equal(r.process_platform,null);assert.equal(r.process_arch,null);assert.equal(r.first_logical_cpu_model,null);
});
