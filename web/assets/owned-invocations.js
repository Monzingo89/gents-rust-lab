// Owner data entry only. The host alone resolves package bytes and source actors.
export function appendInvocation(text, component, fn) {
 const requests=JSON.parse(text);
 if(!Array.isArray(requests)||requests.length>=8)throw Error('Use at most eight invocation inputs');
 if(!/^[a-f0-9]{64}$/.test(component.component_id)||!component.functions.some(f=>f.name===fn.name))throw Error('Select an inspected component function');
 const ids=new Set(requests.map(r=>r.id));let n=1;while(ids.has(`owned-call-${n}`))n++;
 return [...requests,{id:`owned-call-${n}`,component_id:component.component_id,function:fn.name,arguments:[]}];
}
