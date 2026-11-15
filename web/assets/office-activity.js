// Presentation choreography only. Never changes real run state or fabricates
// inference. Telemetry and the durable local researcher have separate sources.
export function officeAssignment(gent,index=0){
 const id=gent.id.toLowerCase(),planner=gent.role==='planner';
 if(id==='ceo')return {floor:10,state:'seated',activity:'Reviewing company status and the efficiency notebook'};
 if(id==='cfo')return {floor:9,state:'seated',activity:'Watching measured inference usage and provider budget'};
 if(id.includes('training'))return {floor:4,pod:0,state:'sleeping',activity:'Recharge cycle · local inference is idle'};
 if(id.includes('law')&&!planner)return {floor:4,pod:1,state:'sleeping',activity:'Resting in a transparent nap pod'};
 if(id.includes('law'))return {floor:8,state:'seated',activity:'Reading the shared procedure library'};
 if(id.includes('pm'))return {floor:planner?7:3,state:'seated',activity:planner?'Studying efficiency notes in the reading hall':'Preparing the next company review in Conference'};
 if(id.includes('compliance')||gent.role==='judge')return {floor:2,state:planner?'seated':'working',activity:'Reviewing local test and safety evidence'};
 if(id.includes('operations')||id.includes('admin'))return {floor:5,state:'working',activity:'Inspecting measured runtime health and failure counters'};
 if(id==='cto')return {floor:5,state:'seated',activity:'Reviewing local capacity and task routing'};
 return {floor:6,state:planner?'seated':'working',activity:'Researching lower-cost local workflows · proposals require validation'};
}

export function elevatorPose(time,floors,reducedMotion=false){
 const phase=reducedMotion?0:(time/6500)%(floors.length*2-2);
 const position=phase<floors.length?phase:floors.length*2-2-phase;
 const index=Math.floor(position),fraction=position-index;
 const start=floors.slice(0,index).reduce((sum,f)=>sum+f.height,0);
 return {index,floor:floors[index].id,y:start+floors[index].height*.5+fraction*(floors[index].height+(floors[index+1]?.height||floors[index].height))*.5};
}
