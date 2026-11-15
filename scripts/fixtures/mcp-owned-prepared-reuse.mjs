// Same finite reuse cases, new owner-preparation ingress. No new task-difficulty claim.
import base from './mcp-owned-weighted-reuse.mjs';
const ids=new Map(base.sourceInputs.map(i=>[i.id,i.arguments.binding_id]));
const sourceInputs=base.sourceInputs.map(i=>({...i,id:ids.get(i.id)})).sort((a,b)=>a.id.localeCompare(b.id));
const files=base.files.map(f=>({...f,path:`owned-${f.owned_component}.rs`}));
const ownedComponents=base.ownedComponents.map(c=>({...c,input:ids.get(c.input),source_path:files.find(f=>f.owned_component===c.owner_declared_review_package_id).path}));
export default {...base,id:'owned-prepared-reuse',title:'Stored scheduling component: owner-prepared inputs and approved source staging',
 ownerPreparation:'retained_component_v1',
 complexity_basis:base.complexity_basis+' This suite qualifies a new preparation ingress on unchanged finite cases; it does not increase the domain difficulty.',
 initial:base.initial+' Use the selected component and prepared invocation inputs; the host retains source custody.',
 requirements:base.requirements.replaceAll('valid-case','weighted-valid').replaceAll('invalid-case','weighted-invalid')+' Source bytes are retained through owner preparation. The evaluator must not manually write the library into the workspace or inject actor-bound invocation requirements into intake. The host derives requirements from the accepted CEO actor and stages exact retained bytes only at approved admission.',
 sourceInputs,planningInputs:sourceInputs,files,ownedComponents,
 dependencies:Object.fromEntries(sourceInputs.map(i=>[i.id,[]])),
 artifacts:base.artifacts.map(a=>({...a,input:ids.get(a.input)})),
 expectedResults:new Map([...base.expectedResults].map(([id,v])=>[ids.get(id),v]))};
