// Review-to-form copy only. No requests, source access, authority or execution.
const canonical=value=>JSON.stringify(Array.isArray(value)?value.map(v=>JSON.parse(canonical(v))):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(k=>[k,JSON.parse(canonical(value[k]))])):value);
export function selectionInvocations(session,currentData){
 const pending=session?.pending_owned_selection;
 if(session?.status!=='selection_review'||!pending||pending.execution_authorized!==false||pending.promotion_authorized!==false)throw Error('A retained discovery-only selection is required');
 if(!/^[a-f0-9]{64}$/.test(pending.proposal_sha256)||pending.catalog_sha256!==session.owned_component_catalog?.catalog_sha256)throw Error('Selection does not match the retained catalog');
 if(canonical(JSON.parse(currentData))!==canonical(session.owned_argument_data))throw Error('Owner data changed; start a new conversation');
 if(!Array.isArray(pending.invocations)||pending.invocations.length<1||pending.invocations.length>8)throw Error('Use one to eight reviewed calls');
 const seen=new Set();
 for(const call of pending.invocations){
  if(Object.keys(call).some(k=>!['id','component_id','function','arguments','argument_slots'].includes(k)))throw Error('Selection contains unsupported preparation fields');
  if(typeof call.id!=='string'||! /^[a-z0-9-]{1,40}$/.test(call.id)||seen.has(call.id))throw Error('Invalid selection call identity');seen.add(call.id);
  const component=session.owned_component_catalog.components.find(c=>c.component_id===call.component_id),fn=component?.functions.find(f=>f.name===call.function);
  if(!fn||!Array.isArray(call.arguments)||call.arguments.length!==fn.arguments?.prefixItems?.length)throw Error('Selection differs from the inspected function interface');
  const slots=call.argument_slots??[];
  if(!Array.isArray(slots)||new Set(slots).size!==slots.length||slots.some(i=>!Number.isSafeInteger(i)||i<0||i>=call.arguments.length||call.arguments[i]!==null))throw Error('Invalid unresolved result slot');
 }
 return structuredClone(pending.invocations);
}
