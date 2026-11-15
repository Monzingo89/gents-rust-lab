// Informational snapshots only. Never permission, runtime admission or task acceptance.
const positive=v=>Number.isSafeInteger(v)&&v>0;
const finite=v=>typeof v==='number'&&Number.isFinite(v)&&v>=0;
export function summarizeComputer(hardware,policy,now=Date.now()){
 const memory=hardware?.memory,devices=hardware?.accelerators?.devices;
 if(!memory||!positive(memory.host_total_bytes)||!positive(memory.effective_limit_bytes)||!finite(memory.effective_available_bytes)||memory.effective_available_bytes>memory.effective_limit_bytes||!positive(memory.observed_at_ms)||memory.observed_at_ms>now+30000||!Array.isArray(devices)||devices.length>8)throw Error('This computer report is incomplete or invalid. Run a fresh local check.');
 const gpus=devices.map(d=>{if(typeof d.name!=='string'||d.name.length>128||/[\x00-\x1f\x7f]/.test(d.name))throw Error('Invalid device description.');return {name:d.name,shared:d.memory_kind==='unified_shared_with_host',dedicated_total_bytes:positive(d.dedicated_total_bytes)?d.dedicated_total_bytes:null};});
 const candidates=policy?.candidates??[];
 if(!Array.isArray(candidates)||candidates.length>16)throw Error('Invalid local model profile.');
 const floors=candidates.map(c=>c.minimum_memory_mib).filter(positive);
 const required=floors.length===candidates.length&&floors.length?Math.max(...floors)*1024**2:null;
 const fresh=now-memory.observed_at_ms<=5*60*1000;
 const status=!fresh?'stale':required===null?'profile_needed':memory.effective_limit_bytes<required?'below_memory_floor':'memory_floor_passed';
 return {status,observed_at_ms:memory.observed_at_ms,total_bytes:memory.host_total_bytes,effective_limit_bytes:memory.effective_limit_bytes,available_bytes:memory.effective_available_bytes,required_bytes:required,gpus,task_seconds:null,model_fit_verified:false,completion_guaranteed:false};
}
export function exportComputerReport(hardware,policy,now=Date.now()){
 summarizeComputer(hardware,policy,now);
 return {type:'metivolve-computer-check',version:1,hardware:{memory:{host_total_bytes:hardware.memory.host_total_bytes,effective_limit_bytes:hardware.memory.effective_limit_bytes,effective_available_bytes:hardware.memory.effective_available_bytes,observed_at_ms:hardware.memory.observed_at_ms},accelerators:{devices:hardware.accelerators.devices.map(d=>({name:d.name,memory_kind:d.memory_kind,dedicated_total_bytes:d.dedicated_total_bytes??null}))}},policy:{candidates:(policy?.candidates??[]).map(c=>({minimum_memory_mib:c.minimum_memory_mib}))},scope:'User-supplied local observation, not independent attestation. No credentials, task content, serial numbers, model paths or execution authority. No model speed has been measured.'};
}
export function importComputerReport(text,now=Date.now()){
 if(typeof text!=='string'||new TextEncoder().encode(text).length>16384)throw Error('Choose a computer report smaller than 16 KB.');
 const value=JSON.parse(text);if(value.type!=='metivolve-computer-check'||value.version!==1)throw Error('Choose a report from the local computer checker.');
 return summarizeComputer(value.hardware,value.policy,now);
}
export function computerMessage(value,now=Date.now()){
 const status=now-value.observed_at_ms>5*60*1000?'stale':value.status;
 const gib=bytes=>(bytes/1024**3).toFixed(1)+' GB';
 const identity=`${value.gpus.map(d=>d.name).join(', ')||'GPU not identified'} · ${gib(value.total_bytes)} system memory · ${gib(value.available_bytes)} currently available.`;
 const messages={stale:'This check is older than five minutes. Refresh it in the local app.',profile_needed:'Computer detected. Select a reviewed local model before checking its memory needs.',below_memory_floor:`This setup is below the selected profile’s ${gib(value.required_bytes)} memory floor. Choose a smaller reviewed model or another computer.`,memory_floor_passed:`The selected profile’s ${gib(value.required_bytes)} basic memory check passes. Model fit and task completion still need local checks.`};
 return {title:messages[status],detail:identity+' Apple unified memory is shared, not extra GPU memory. Time remains unmeasured until the task, model and measured speed are known.'};
}
