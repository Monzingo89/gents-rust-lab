// Metadata only. Server ownership and immutable session capture are authoritative.
export function ownedDiscovery({catalog, choices, status, planningStatus, fetchCatalog, locked, addInvocation}) {
 let generation=0, entries=[], selected=new Set(), retained=null;
 const ids=()=>[...selected].sort();
 const report=text=>{status.textContent=text;planningStatus.textContent=text;};
 function render(){
  const shown=retained??entries;
  catalog.replaceChildren(...entries.map(entry=>{
   const details=document.createElement('details'),summary=document.createElement('summary'),body=document.createElement('pre');
   summary.textContent=`${entry.origin_fixture} · ${(entry.functions??[]).map(f=>f.name).join(', ')}`;
   body.textContent=JSON.stringify(entry,null,2);details.append(summary,body);return details;
  }));
  choices.replaceChildren(...shown.map(entry=>{
   const label=document.createElement('label'),box=document.createElement('input'),text=document.createElement('span');label.className='consent';box.type='checkbox';box.value=entry.component_id;box.checked=selected.has(entry.component_id);box.disabled=locked();
   text.textContent=`${entry.origin_fixture} · ${(entry.functions??[]).map(f=>f.name).join(', ')} · ${entry.component_id.slice(0,12)}`;
   box.onchange=()=>{if(locked()){render();return;}if(box.checked&&selected.size>=4){box.checked=false;report('Select at most four interfaces for one conversation.');return;}if(box.checked)selected.add(entry.component_id);else selected.delete(entry.component_id);render();};
   label.append(box,text);
   const section=document.createElement('section');section.append(label);
   if(selected.has(entry.component_id))for(const fn of entry.functions??[]){
    const details=document.createElement('details'),summary=document.createElement('summary'),schema=document.createElement('pre'),button=document.createElement('button');
    summary.textContent=`${fn.name} input/result schemas`;schema.textContent=JSON.stringify(fn,null,2);button.type='button';button.textContent=`Add invocation input for ${fn.name}`;
    button.onclick=()=>{try{addInvocation(entry,fn);}catch(e){report(e.message);}};
    details.append(summary,schema,button);section.append(details);
   }
   return section;
  }));
 }
 function reset(){generation++;entries=[];selected.clear();retained=null;catalog.replaceChildren();choices.replaceChildren();report('Refresh to inspect this owner’s locally stored component interfaces.');}
 function restore(snapshot){
  retained=snapshot?.components??[];selected=new Set(retained.map(e=>e.component_id));render();
 }
 async function load(){
  const ticket=++generation;report('Inspecting owned local interfaces…');
  try{
   const view=await fetchCatalog();if(ticket!==generation)return;
   if(view.execution_authorized!==false||view.promotion_authorized!==false||!Array.isArray(view.components))throw Error('Invalid information-only catalog');
   entries=view.components;
   if(retained===null)selected=new Set(ids().filter(id=>entries.some(entry=>entry.component_id===id)));
   render();report(`${entries.length} owner-visible components · interfaces only · execution requires frozen bindings and approval`);
  }catch(e){if(ticket!==generation)return;entries=[];if(retained===null)selected.clear();render();report(`Component information unavailable: ${e.message}`);}
 }
 return {ids,reset,restore,load,render};
}
