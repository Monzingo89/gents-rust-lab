// A lost HTTP response is not permission to replay a planning POST.
export class PlanningObservationUncertain extends Error {}
export async function planningTurn({request,path,body,session,onObservation=async()=>{},pause=()=>new Promise(resolve=>setTimeout(resolve,2000))}){
 try{return await request(path,body);}catch(error){
  if(Number.isInteger(error.status))throw error;
  await onObservation({kind:'planning_response_lost',session,expected_revision:body.expected_revision,message:'Retaining issued turn; polling only, no POST replay'});
 }
 for(;;){
  let state;
  try{state=await request(path);}catch(error){
   if(Number.isInteger(error.status)&&error.status>=400&&error.status<500)throw new PlanningObservationUncertain('Planning state unavailable; original turn remains uncertain and must not be replayed');
   await pause();continue;
  }
  if(state.id!==session)throw new PlanningObservationUncertain('Planning observation session mismatch; retain original turn');
  if(state.revision!==body.expected_revision+1)throw new PlanningObservationUncertain('Planning revision differs from the issued turn; retain original evidence and reconcile without replay');
  if(state.status==='generating'){await pause();continue;}
  if(!['question','selection_review','ready','failed'].includes(state.status))throw new PlanningObservationUncertain('Unknown planning state; retain original turn');
  await onObservation({kind:'planning_response_recovered',session,revision:state.revision,status:state.status,replayed:false});
  return state;
 }
}
