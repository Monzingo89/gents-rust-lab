// Operator-provided artwork. Appearance is independent of role permissions,
// model availability, and execution. Changing a portrait never changes a model.
export const characters = [
  {id:'fox',name:'Fox',src:new URL('./assets/gents/fox.jpg',import.meta.url).href,color:'#bf7042'},
  {id:'crystal',name:'Crystal sprite',src:new URL('./assets/gents/crystal.jpg',import.meta.url).href,color:'#9d88b1'},
  {id:'moth',name:'Moth',src:new URL('./assets/gents/moth.jpg',import.meta.url).href,color:'#c78da8'},
  {id:'dwarf',name:'Dwarf',src:new URL('./assets/gents/dwarf.jpg',import.meta.url).href,color:'#b47a51'},
  {id:'robot',name:'Clockwork robot',src:new URL('./assets/gents/robot.jpg',import.meta.url).href,color:'#b59c61'},
  {id:'dragon',name:'Water dragon',src:new URL('./assets/gents/dragon.jpg',import.meta.url).href,color:'#7b9eb1'},
  {id:'golem',name:'Stone golem',src:new URL('./assets/gents/golem.jpg',import.meta.url).href,color:'#a49179'},
  {id:'alien',name:'Starlight scholar',src:new URL('./assets/gents/alien.jpg',import.meta.url).href,color:'#aaa1bc'},
  {id:'faun',name:'Faun',src:new URL('./assets/gents/faun.jpg',import.meta.url).href,color:'#bb8962'},
  {id:'elf',name:'Elf',src:new URL('./assets/gents/elf.jpg',import.meta.url).href,color:'#88749f'},
  {id:'mushroom',name:'Mushroom',src:new URL('./assets/gents/mushroom.jpg',import.meta.url).href,color:'#be7e71'},
  {id:'slime',name:'Jade spirit',src:new URL('./assets/gents/slime.jpg',import.meta.url).href,color:'#9ab19b'},
  {id:'moon',name:'Luna · Moon spirit',src:new URL('./assets/gents/moon.png',import.meta.url).href,color:'#e2cba6'},
  {id:'octopus',name:'Coral · Octopus',src:new URL('./assets/gents/octopus.png',import.meta.url).href,color:'#ee978a'},
  {id:'emerald',name:'Ember · Emerald dragon',src:new URL('./assets/gents/emerald.png',import.meta.url).href,color:'#58ae91'},
  {id:'cactus',name:'Sage · Cactus',src:new URL('./assets/gents/cactus.png',import.meta.url).href,color:'#b8c37e'},
  {id:'raven',name:'Onyx · Raven',src:new URL('./assets/gents/raven.png',import.meta.url).href,color:'#8595d6'},
  {id:'stag',name:'Rowan · Stag',src:new URL('./assets/gents/stag.png',import.meta.url).href,color:'#d8a77b'},
  {id:'yeti',name:'Boreal · Yeti',src:new URL('./assets/gents/yeti.png',import.meta.url).href,color:'#b8d9eb'},
  {id:'lion',name:'Atlas · Lion',src:new URL('./assets/gents/lion.png',import.meta.url).href,color:'#e2af68'},
];

export const spriteSheets=Array.from({length:5},(_,i)=>new URL(`./assets/gents/fullbody-${i+1}-v1.png`,import.meta.url).href);
export function spriteCell(index,width,height){
 const w=Math.floor(width/2),h=Math.floor(height/2),cell=index%4;
 return {x:(cell%2)*w,y:Math.floor(cell/2)*h,w,h};
}
// Tight alpha bounds keep differently shaped ears/wings at their natural ratio.
export function opaqueBounds(data,width,cell,threshold=24){
 let left=cell.x+cell.w,top=cell.y+cell.h,right=-1,bottom=-1;
 for(let y=cell.y;y<cell.y+cell.h;y++)for(let x=cell.x;x<cell.x+cell.w;x++)if(data[(y*width+x)*4+3]>threshold){left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);}
 return right<0?cell:{x:left,y:top,w:right-left+1,h:bottom-top+1};
}
export function previewPosition(anchor,stage,preview){
 const gap=18,pad=12;
 const left=anchor.right+gap+preview.width<=stage.width-pad?anchor.right+gap:anchor.left-gap-preview.width;
 return {left:Math.max(pad,Math.min(left,stage.width-preview.width-pad)),top:Math.max(pad,Math.min(anchor.top,stage.height-preview.height-pad))};
}
export function spriteContains(hit,x,y){
 if(!hit||x<hit.x||x>=hit.x+hit.w||y<hit.y||y>=hit.y+hit.h)return false;
 if(!hit.sprite)return true;
 const f=hit.sprite,px=Math.floor(f.x+(x-hit.x)/hit.w*f.w),py=Math.floor(f.y+(y-hit.y)/hit.h*f.h);
 return f.data[(py*f.width+px)*4+3]>24;
}

export function modelKey(model='') {
  const text=String(model).toLowerCase().replace(/^(local|mlx|mock)\s*\/\s*/,'').replace(/admin\s+judge/g,'').trim();
  // Same weights under display names / paths / quantizations share one look.
  for(const [pattern,key] of [[/qwen3[-_ ]coder[-_ ]30b/,'qwen3-coder-30b-a3b'],[/qwen3[-_ ]vl[-_ ]8b/,'qwen3-vl-8b'],[/qwen3[-_ ]8b/,'qwen3-8b'],[/phi[-_ ]4\b/,'phi-4'],[/gemma[-_ ]4[-_ ]?e4b/,'gemma-4-e4b']])if(pattern.test(text))return key;
  return text||'unassigned';
}

const defaults={'qwen3-8b':'fox','phi-4':'crystal','qwen3-coder-30b-a3b':'dwarf','qwen3-vl-8b':'elf','gemma-4-e4b':'robot','training worker':'moth','operations coordinator':'golem','unassigned':'slime'};
export function avatarForModel(model,overrides={}) {
  const key=modelKey(model);
  const chosen=characters.find(c=>c.id===overrides[key])||characters.find(c=>c.id===defaults[key]);
  if(chosen)return chosen;
  // Stable fallback for a new model, never random per render or per role.
  let hash=2166136261;
  for(const char of key)hash=Math.imul(hash^char.charCodeAt(0),16777619)>>>0;
  return characters[hash%characters.length];
}

// Explicit position identities never depend on the assigned inference model.
// The generic engineering runtime is the web-engineer position. Every base
// position has its own species; split roles use distinct identities as well.
export const positionStyles={
 'ceo':['lion','#e2af68'], 'cto':['crystal','#8bbabf'],
 'law':['alien','#a5a3d6'], 'hr':['faun','#e0a48e'],
 'cfo':['moon','#e2cba6'], 'accountant':['mushroom','#bd9a87'],
 'pm':['octopus','#ee978a'], 'lead-architect':['emerald','#58ae91'],
 'web-engineer':['dwarf','#dc9457'], 'mobile-engineer':['robot','#83bcbb'],
 'vr-engineer':['slime','#a1bfaa'], 'database-engineer':['cactus','#b8c37e'],
 'backend-engineer':['dragon','#89b6cf'], 'ui-ux-engineer':['elf','#a398d6'],
 'cloud-engineer':['moth','#cba5ca'], 'ai-engineer':['raven','#8595d6'],
 'compliance-security-qa':['fox','#d89b6e'], 'admin-judge':['golem','#b2b9b6'],
 'operations':['yeti','#b8d9eb'], 'training':['stag','#d8a77b'],
};
export function positionKey(id=''){
 const key=String(id).trim().toLowerCase().split(/\s+-\s+/)[0]
  .replace(/\s+gent$/,'').replace(/&/g,' ').replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'');
 return ({'routing-judge':'cto','completion-judge':'compliance-security-qa',
  'compliance-security-qa-engineer':'compliance-security-qa','engineer':'web-engineer'})[key]||key||'unassigned';
}
export const appearanceKey=(id,phase=null)=>`${positionKey(id)}:${phase||'position'}`;
export function avatarForPosition(id,phase=null,overrides={}){
 const key=positionKey(id),style=positionStyles[key];
 let fallback=style?.[0]||avatarForModel(`position:${key}`).id;
 if(phase==='planner'&&key==='law')fallback='mushroom';
 if(phase==='planner'&&key==='cto')fallback='moon';
 if(phase==='planner'&&key==='pm')fallback='faun';
 if(phase==='planner'&&key==='lead-architect')fallback='dragon';
 if(phase==='planner'&&key==='web-engineer')fallback='raven';
 if(phase==='planner'&&key==='compliance-security-qa')fallback='cactus';
 const selected=overrides[appearanceKey(id,phase)]||overrides[appearanceKey(id)];
 const character=characters.find(c=>c.id===selected)||characters.find(c=>c.id===fallback);
 return {...character,position:key,appearanceKey:appearanceKey(id,phase),accent:style?.[1]||character.color,
  accessory:phase==='planner'?'notebook':phase==='worker'?'toolcase':'pin'};
}

export function uniqueCrewLooks(crew,overrides={}){
 const result=new Map(),used=new Set();
 // An explicit phase choice wins, then existing defaults, then an unused face.
 const ordered=[...crew].sort((a,b)=>Number(!!overrides[appearanceKey(b.id,b.role)])-Number(!!overrides[appearanceKey(a.id,a.role)]));
 for(const agent of ordered){
  const key=appearanceKey(agent.id,agent.role);if(result.has(key))continue;
  const preferred=avatarForPosition(agent.id,agent.role,overrides);
  const chosen=!used.has(preferred.id)?preferred:characters.find(c=>!used.has(c.id));
  if(!chosen){result.set(key,null);continue;} // Never silently clone a face.
  used.add(chosen.id);result.set(key,{...preferred,...chosen});
 }
 return result;
}
