// Portable observer metadata; never a model-fit or permission decision.
import * as os from 'node:os';
const observe=read=>{try{return read();}catch{return null;}};
const positive=value=>Number.isSafeInteger(value)&&value>0?value:null;
const label=value=>typeof value==='string'&&value.trim()&&value.length<=128&&!/[\x00-\x1f\x7f]/.test(value)?value.trim():null;
export function evaluationHost(probe=os){
 const cpus=observe(()=>probe.cpus());
 return {schema_version:1,observed_at_ms:Date.now(),
  process_platform:label(observe(()=>probe.platform())),process_arch:label(observe(()=>probe.arch())),
  os_release:label(observe(()=>probe.release())),node_version:process.version,
  first_logical_cpu_model:label(cpus?.[0]?.model),logical_cpu_count:Array.isArray(cpus)?positive(cpus.length):null,
  process_available_parallelism:positive(observe(()=>probe.availableParallelism())),
  os_reported_memory_bytes:positive(observe(()=>probe.totalmem())),
  scope:'Observer OS snapshot only; missing readings remain unknown. OS memory is not a cgroup/process/GPU allowance. No hostname, device ID, credentials, GPU identity, native inference attestation, capacity grant or automatic model selection.'};
}
