import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {characters,modelKey,avatarForModel,positionStyles,positionKey,appearanceKey,avatarForPosition,uniqueCrewLooks,spriteSheets,spriteCell,opaqueBounds,previewPosition,spriteContains} from '../web/gent-avatars.js';
import {towerFloors,reserveTowerSeats,detailStationRect,detailPodRect,floorWalkPose,interiorPoint,interiorPodRect,roomDesigns,crewKey,rectsOverlap} from '../web/assets/tower-layout.js';
import {renderTower} from '../web/assets/tower-scene.js';

test('all twenty supplied identities exist locally, including the eight new originals',()=>{
 assert.equal(characters.length,20);
 assert.equal(new Set(characters.map(c=>c.id)).size,20);
 for(const character of characters){const bytes=readFileSync(new URL(character.src));assert.ok(bytes.length>1000);assert.ok(bytes.readUInt16BE()===0xffd8||bytes.subarray(1,4).toString()==='PNG');}
});
test('five full-body sheets cover all twenty characters in separate equal cells',()=>{
 assert.equal(spriteSheets.length,5);
 for(const src of spriteSheets){const bytes=readFileSync(new URL(src));assert.equal(bytes.subarray(1,4).toString(),'PNG');assert.ok(bytes.length>10000);}
 assert.deepEqual(spriteCell(0,1280,1280),{x:0,y:0,w:640,h:640});
 assert.deepEqual(spriteCell(7,1280,1280),{x:640,y:640,w:640,h:640});
});
test('alpha bounds crop each silhouette without sampling a neighboring cell',()=>{
 const data=new Uint8ClampedArray(8*8*4);for(const [x,y] of [[1,1],[2,3],[6,6]])data[(y*8+x)*4+3]=255;
 assert.deepEqual(opaqueBounds(data,8,spriteCell(0,8,8)),{x:1,y:1,w:2,h:3});
 assert.deepEqual(opaqueBounds(data,8,spriteCell(3,8,8)),{x:6,y:6,w:1,h:1});
});
test('hover previews flip and clamp inside the office viewport',()=>{
 assert.deepEqual(previewPosition({left:100,right:160,top:40},{width:900,height:500},{width:292,height:160}),{left:178,top:40});
 assert.deepEqual(previewPosition({left:800,right:860,top:480},{width:900,height:500},{width:292,height:160}),{left:490,top:328});
 assert.deepEqual(previewPosition({left:10,right:30,top:-40},{width:280,height:200},{width:256,height:160}),{left:12,top:12});
});
test('sprite selection follows visible alpha, not the transparent rectangular corners',()=>{
 const data=new Uint8ClampedArray(4*4*4);data[(1*4+2)*4+3]=255;
 const hit={x:10,y:20,w:40,h:40,sprite:{x:0,y:0,w:4,h:4,width:4,data}};
 assert.equal(spriteContains(hit,35,35),true);
 assert.equal(spriteContains(hit,15,25),false);
 assert.equal(spriteContains(hit,50,35),false);
 assert.equal(spriteContains(hit,9,35),false);
});
test('renderer separates the glazed building, one-floor interior and sleep-pod views',()=>{
 const crew=[{id:'Engineer Gent',role:'worker',busy:true,activity:'Working on mission'},{id:'CEO',role:'oversight',busy:false}],calls=[];
 const ctx=new Proxy({canvas:{width:1600,height:1480},createLinearGradient:()=>({addColorStop(){}}),createRadialGradient:()=>({addColorStop(){}}),fillText:value=>calls.push(value),drawImage:(...args)=>calls.push(args)}, {get:(t,p)=>p in t?t[p]:()=>{}});
 const frames=new Map(characters.map(c=>[c.id,{image:c.id,x:0,y:0,w:100,h:180}]));
 const stats=renderTower(ctx,{crew,characterFor:avatarForPosition,spriteFrames:frames});
 assert.deepEqual(stats,{active:1,resting:1,unassigned:0});
 assert.equal(ctx.canvas.floorHits.length,6);assert.equal(ctx.canvas.gentLayout.length,0);assert.equal(ctx.canvas.height,1180);
 calls.length=0;renderTower(ctx,{crew,characterFor:avatarForPosition,spriteFrames:frames,filter:'4',time:2000});
 assert.equal(ctx.canvas.gentLayout.length,1);assert.equal(ctx.canvas.height,1000);assert.equal(ctx.canvas.gentLayout[0].resting,false);
 assert.ok(calls.indexOf('Working on mission')>calls.findIndex(c=>typeof c==='string'&&c.includes('Architecture & production')));
 assert.equal(calls.filter(Array.isArray).length,3,'Walking rig draws both legs and the untouched upper body');
 calls.length=0;renderTower(ctx,{crew,characterFor:avatarForPosition,spriteFrames:frames,filter:'2'});
 assert.equal(ctx.canvas.gentLayout.length,1);assert.equal(ctx.canvas.gentLayout[0].resting,true);
 assert.equal(calls.filter(Array.isArray)[0][4],Math.floor(180*.57));
});
test('model aliases share the same character across roles, display names and quantizations',()=>{
 for(const alias of ['Qwen3-8B','local / Qwen3-8B Admin Judge','models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf']){
  assert.equal(modelKey(alias),'qwen3-8b');assert.equal(avatarForModel(alias).id,'fox');
 }
 assert.equal(avatarForModel('Phi-4').id,'crystal');
 assert.equal(avatarForModel('Qwen3-Coder-30B-A3B-Instruct').id,'dwarf');
 assert.equal(avatarForModel('Qwen3-VL-8B-Instruct').id,'elf');
});
test('appearance overrides are shared by model, without changing unrelated models',()=>{
 const choices={'qwen3-8b':'mushroom'};
 assert.equal(avatarForModel('Qwen3-8B Admin Judge',choices).id,'mushroom');
 assert.equal(avatarForModel('Phi-4',choices).id,'crystal');
 assert.equal(avatarForModel('Qwen3-8B',{'qwen3-8b':'invalid-asset'}).id,'fox');
 assert.equal(avatarForModel('new-model-42').id,avatarForModel('new-model-42').id);
});
test('every configured position has a distinct character and uniform combination',()=>{
 const organization=JSON.parse(readFileSync(new URL('../config/organization.json',import.meta.url),'utf8'));
 const ids=[...organization.roles.map(r=>r.id),'Admin Judge','Operations Gent','Training Gent'];
 const looks=ids.map(id=>avatarForPosition(id));
 assert.equal(ids.length,20);
 assert.equal(new Set(looks.map(c=>`${c.id}:${c.accent}`)).size,ids.length);
 for(const look of looks){assert.ok(positionStyles[look.position]);assert.ok(characters.some(c=>c.id===look.id));}
 for(const role of organization.roles)assert.equal(positionKey(role.title),role.id);
 const runtime=JSON.parse(readFileSync(new URL('../config/company.local.json',import.meta.url),'utf8'));
 const active=runtime.agents.flatMap(a=>['planner','worker'].map(phase=>avatarForPosition(a.id,phase)))
  .concat(['CEO','Admin Judge','Operations Gent','Training Gent'].map(id=>avatarForPosition(id)));
 assert.equal(new Set(active.map(c=>`${c.id}:${c.accent}:${c.accessory}`)).size,active.length);
 assert.equal(new Set(active.map(c=>c.id)).size,active.length,'No two active crew members may reuse a face');
 for(const id of ['moon','octopus','emerald','cactus','raven','stag','yeti','lion'])assert.ok(active.some(c=>c.id===id));
});
test('position aliases match their roster identities, not the shared model',()=>{
 for(const [name,key] of [['Law Gent','law'],['PM Gent','pm'],['Lead Architect Gent','lead-architect'],['Routing Judge','cto'],['Completion Judge','compliance-security-qa'],['Compliance Security & QA Engineer','compliance-security-qa'],['Operations Gent','operations']]){
  assert.equal(positionKey(name),key);assert.deepEqual(avatarForPosition(name),avatarForPosition(key));
 }
 assert.equal(avatarForPosition('CEO').id,'lion');
 assert.equal(avatarForPosition('CTO').id,'crystal');
 assert.equal(avatarForPosition('Admin Judge').id,'golem');
});
test('position and split-role choices persist independently without cross-model bleed',()=>{
 const choices={[appearanceKey('PM Gent')]:'mushroom',[appearanceKey('Law Gent','planner')]:'robot'};
 assert.equal(avatarForPosition('pm','worker',choices).id,'mushroom');
 assert.equal(avatarForPosition('Law Gent','planner',choices).id,'robot');
 assert.equal(avatarForPosition('law','worker',choices).id,'alien');
 assert.equal(avatarForPosition('CTO',null,choices).id,'crystal');
 assert.equal(avatarForPosition('pm','planner').accessory,'notebook');
 assert.equal(avatarForPosition('pm','worker').accessory,'toolcase');
 assert.notEqual(avatarForPosition('law','planner').id,avatarForPosition('law','worker').id);
 assert.equal(avatarForPosition('PM',null,{'pm:position':'missing'}).id,'octopus');
 assert.deepEqual(avatarForPosition('future-role'),avatarForPosition('future-role'));
 const source=readFileSync(new URL('../web/app.js',import.meta.url),'utf8');
 assert.ok(!source.includes('gents:model-characters:v1'));
 assert.ok(source.includes('gents:unique-crew:v2'));
 assert.ok(!source.includes('characterFor(model)'));
});
test('the scene allocator rejects cloned choices and never silently overflows its twenty identities',()=>{
 const crew=Array.from({length:22},(_,i)=>({id:`new-role-${i}`,role:'worker'}));
 const choices=Object.fromEntries(crew.map(s=>[appearanceKey(s.id,s.role),'fox']));
 const looks=uniqueCrewLooks(crew,choices),assigned=[...looks.values()].filter(Boolean);
 assert.equal(assigned.length,20);assert.equal(new Set(assigned.map(c=>c.id)).size,20);
 assert.equal([...looks.values()].filter(c=>c===null).length,2);
 assert.deepEqual([...uniqueCrewLooks(crew,choices)],[...looks]);
});
test('reserved stations, pods and room headers cannot overlap for idle, mixed or all-working crews',()=>{
 const runtime=JSON.parse(readFileSync(new URL('../config/company.local.json',import.meta.url),'utf8'));
 const crew=runtime.agents.flatMap(a=>['planner','worker'].map(role=>({id:a.id,role}))).concat(['CEO','Admin Judge','Operations Gent','Training Gent'].map(id=>({id,role:'special'})));
 const seats=reserveTowerSeats(crew);
 assert.equal(towerFloors.length,6);assert.equal(seats.size,crew.length);
 for(const mode of ['idle','mixed','working']){
  const rects=crew.map((s,i)=>{const seat=seats.get(crewKey(s)),busy=mode==='working'||mode==='mixed'&&i%2===0;return{...busy?detailStationRect(seat):detailPodRect(seat.pod),floor:busy?seat.floor:seat.podFloor};});
  for(let i=0;i<rects.length;i++)for(let j=i+1;j<rects.length;j++)if(rects[i].floor===rects[j].floor)assert.equal(rectsOverlap(rects[i],rects[j]),false,`${mode}: ${i} and ${j}`);
  for(const r of rects){assert.ok(r.x>126&&r.x+r.w<1474);assert.ok(r.y>192&&r.y+r.h<854);}
 }
});
test('large sprites walk inside non-overlapping lanes and reduced-motion poses are stationary',()=>{
 const crew=Array.from({length:8},(_,i)=>({id:`Engineer ${i}`,role:'worker'})),seats=reserveTowerSeats(crew);
 const reservations=crew.map(s=>seats.get(crewKey(s))),rects=reservations.map(detailStationRect);
 for(let i=0;i<rects.length;i++)for(let j=i+1;j<rects.length;j++)if(reservations[i].floor===reservations[j].floor)assert.equal(rectsOverlap(rects[i],rects[j]),false);
 for(const [i,r]of rects.entries())for(let t=0;t<30000;t+=100){const p=floorWalkPose(r,t,i);assert.ok(p.x-87>=r.x&&p.x+87<=r.x+r.w);assert.ok(p.y-170>=r.y&&p.y+10<=r.y+r.h);}
 assert.notEqual(floorWalkPose(rects[0],1000).x,floorWalkPose(rects[0],5000).x);
 assert.equal(floorWalkPose(rects[0],1000,0,true).x,floorWalkPose(rects[0],5000,0,true).x);
 const pods=Array.from({length:10},(_,i)=>detailPodRect(i));
 for(let i=0;i<pods.length;i++)for(let j=i+1;j<pods.length;j++)assert.equal(rectsOverlap(pods[i],pods[j]),false);
});
test('walking preview never changes actual model activity counts or busy state',()=>{
 const crew=[{id:'Engineer Gent',role:'worker',busy:false}],ctx=new Proxy({canvas:{},createLinearGradient:()=>({addColorStop(){}}),createRadialGradient:()=>({addColorStop(){}})},{get:(t,p)=>p in t?t[p]:()=>{}});
 const stats=renderTower(ctx,{crew,characterFor:avatarForPosition,spriteFrames:new Map(),filter:'4',preview:true,time:1000});
 assert.deepEqual(stats,{active:0,resting:1,unassigned:0});assert.equal(crew[0].busy,false);assert.equal(ctx.canvas.gentLayout[0].preview,true);
});
test('floor interiors use one angled camera, eight distinct room designs, and matching recharge suites',()=>{
 assert.equal(new Set(Object.values(roomDesigns).map(d=>d.id)).size,8);
 assert.equal(new Set(Object.values(roomDesigns).map(d=>d.material)).size,8);
 const a=interiorPoint(126,180),b=interiorPoint(1474,180),front=interiorPoint(126,854);
 assert.ok(b.x>a.x&&b.y<a.y&&front.x>a.x&&front.y>a.y,'Ground plane must visibly recede in both axes');
 assert.equal(interiorPoint(126,180,100).y,a.y-100,'Vertical walls and upright actors share the ground plane');
 const ctx=new Proxy({canvas:{},createLinearGradient:()=>({addColorStop(){}}),createRadialGradient:()=>({addColorStop(){}})},{get:(t,p)=>p in t?t[p]:()=>{}});
 for(const f of towerFloors){renderTower(ctx,{crew:[],characterFor:avatarForPosition,spriteFrames:new Map(),filter:String(f.id)});assert.equal(ctx.canvas.interiorDesign.camera,'oblique-35deg');assert.equal(ctx.canvas.interiorDesign.floor,f.id);}
 renderTower(ctx,{crew:[],characterFor:avatarForPosition,spriteFrames:new Map(),filter:'1'});const design=ctx.canvas.interiorDesign.rooms;
 renderTower(ctx,{crew:[],characterFor:avatarForPosition,spriteFrames:new Map(),filter:'2'});assert.deepEqual(ctx.canvas.interiorDesign.rooms,design);
 for(let i=0;i<10;i++){const p=interiorPodRect(i);assert.equal(p.w,198);assert.equal(p.h,181);for(let j=i+1;j<10;j++)assert.equal(rectsOverlap(p,interiorPodRect(j)),false);}
});
test('projected sprite hit targets stay separated across walking, mixed and resting crews',()=>{
 const runtime=JSON.parse(readFileSync(new URL('../config/company.local.json',import.meta.url),'utf8'));
 const crew=runtime.agents.flatMap(a=>['planner','worker'].map(role=>({id:a.id,role}))).concat(['CEO','Admin Judge','Operations Gent','Training Gent'].map(id=>({id,role:'special'})));
 const ctx=new Proxy({canvas:{},createLinearGradient:()=>({addColorStop(){}}),createRadialGradient:()=>({addColorStop(){}})},{get:(t,p)=>p in t?t[p]:()=>{}});
 const frames=new Map(characters.map(c=>[c.id,{image:c.id,x:0,y:0,w:180,h:180}]));
 for(const mode of ['working','mixed','idle']){
  for(const [i,s]of crew.entries())s.busy=mode==='working'||mode==='mixed'&&i%2===0;
  for(let t=0;t<15000;t+=500)for(const floor of towerFloors){
   for(const s of crew){s.walkClock=t;s.lastVisualTime=t;}
   renderTower(ctx,{crew,characterFor:avatarForPosition,spriteFrames:frames,filter:String(floor.id),time:t});
   const hits=ctx.canvas.gentLayout.map(s=>s.hit);
   for(let i=0;i<hits.length;i++){const h=hits[i];assert.ok(h.x>40&&h.x+h.w<1560&&h.y>120&&h.y+h.h<900);for(let j=i+1;j<hits.length;j++)assert.equal(rectsOverlap(h,hits[j]),false,`${mode}: floor ${floor.id}, ${i} and ${j}`);}
  }
 }
});
