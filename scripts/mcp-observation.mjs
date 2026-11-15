// Read-only execution snapshots. Never overwrite terminal admission evidence.
import {writeFile,rename,unlink} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
import {renderWorkflowTranscript} from './mcp-transcript.mjs';

export async function saveObservation({folder,fixture,report,plan,saved,now=Date.now()}){
 if(saved.run_id!==report.run||saved.company_id!==report.company)throw Error('Observation scope mismatch');
 const encoded=JSON.stringify(saved,null,2);
 const observation={...report,status:'OBSERVATION_ONLY',observation_only:true,observed_at_ms:now,runtime_status:saved.status,independently_admitted:false,event_count:saved.events.length,events_sha256:createHash('sha256').update(encoded).digest('hex')};
 for(const [name,body] of [['observation-events.json',encoded],['observation.html',renderWorkflowTranscript({fixture,report:observation,plan,saved})]]){
  const temporary=new URL(`.${randomUUID()}.tmp`,folder);
  try{await writeFile(temporary,body,{mode:0o600,flag:'wx'});await rename(temporary,new URL(name,folder));}
  finally{await unlink(temporary).catch(error=>{if(error.code!=='ENOENT')throw error;});}
 }
 return observation;
}
