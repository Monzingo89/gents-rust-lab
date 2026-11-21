import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {officeAssignment,elevatorPose} from '../web/assets/office-activity.js';
import {screenDefinitions,screenValue,dashboardDetail} from '../web/assets/office-dashboards.js';
import {towerFloors,facadeHeight,reserveTowerSeats,crewKey,rectsOverlap} from '../web/assets/tower-layout.js';
import {renderTower} from '../web/assets/tower-scene.js';
import {uniqueCrewLooks,appearanceKey} from '../web/gent-avatars.js';
const runtime=JSON.parse(readFileSync(new URL('../config/company.local.json',import.meta.url),'utf8'));
const crew=()=>runtime.agents.flatMap(a=>['planner','worker'].map(role=>({id:a.id,role,busy:false}))).concat(['Admin Judge','Operations Gent','Training Gent','CEO','CFO'].map(id=>({id,role:'special',busy:false})));
const context=()=>new Proxy({canvas:{},createLinearGradient:()=>({addColorStop(){}}),createRadialGradient:()=>({addColorStop(){}})},{get:(target,key)=>key in target?target[key]:()=>{}});

test('activity gives every distinct Gent a stable room, two sleepers and both executives',()=>{
 const people=crew(),assignments=people.map(officeAssignment),seats=reserveTowerSeats(people,{tour:true});
 assert.equal(assignments.filter(a=>a.state==='sleeping').length,2);
 assert.equal(officeAssignment({id:'CEO'}).floor,10);assert.equal(officeAssignment({id:'CFO'}).floor,9);
 assert.equal(officeAssignment({id:'CEO'}).state,'seated');assert.equal(officeAssignment({id:'CFO'}).state,'seated');
 assert.ok(assignments.some(a=>a.state==='working'));assert.ok(assignments.some(a=>a.state==='seated'));
 for(const p of people)assert.ok(seats.get(crewKey(p)));
 assert.equal(new Set([...uniqueCrewLooks(people)].map(([,look])=>look.id)).size,people.length);
 assert.ok(people.every(p=>p.busy===false),'choreography must not mark real inference busy');
});
test('mixed occupancy is rendered and hoverable in both exterior and every room',()=>{
 const people=crew(),looks=uniqueCrewLooks(people),ctx=context(),characterFor=(id,role)=>looks.get(appearanceKey(id,role));
 renderTower(ctx,{crew:people,characterFor,spriteFrames:new Map(),tour:true,time:1000});
 assert.equal(ctx.canvas.gentLayout.length,people.length);assert.equal(ctx.canvas.gentLayout.filter(p=>p.resting).length,2);
 const exterior=ctx.canvas.gentLayout;let total=0;
 for(const f of towerFloors){renderTower(ctx,{crew:people,characterFor,spriteFrames:new Map(),filter:String(f.id),tour:true,time:1000});total+=ctx.canvas.gentLayout.length;
  for(const p of ctx.canvas.gentLayout){assert.equal(p.floor,f.id);assert.ok(p.hit.w>0&&p.hit.h>0);}
  const hits=ctx.canvas.gentLayout;for(let i=0;i<hits.length;i++)for(let j=i+1;j<hits.length;j++)assert.equal(rectsOverlap(hits[i].hit,hits[j].hit),false,`floor ${f.id}: ${hits[i].key} / ${hits[j].key}`);
 }
 assert.equal(total,people.length);assert.ok(exterior.some(g=>g.state==='seated'));
});
test('exterior room proportions are no longer flattened and elevator traverses floors',()=>{
 assert.ok(towerFloors.every(f=>900/f.height<2.1));assert.ok(facadeHeight>5000);
 const stops=new Set(Array.from({length:11},(_,i)=>elevatorPose(i*6500,towerFloors).floor));assert.equal(stops.size,11);
 assert.notEqual(elevatorPose(0,towerFloors).y,elevatorPose(30000,towerFloors).y);
 assert.deepEqual(elevatorPose(0,towerFloors,true),elevatorPose(30000,towerFloors,true));
});
test('role-specific screens use real values, preserve unknown sensors and expose provenance',()=>{
 assert.deepEqual(screenDefinitions(9).map(s=>s.id),['taskbudget']);assert.equal(screenDefinitions(5).length,11);assert.equal(screenDefinitions(10).length,6);
 for(const f of towerFloors)assert.ok(screenDefinitions(f.id).length);
 const gpu=screenDefinitions(5).find(s=>s.id==='gpu'),budget=screenDefinitions(5).find(s=>s.id==='budget');
 assert.equal(screenValue(gpu,{available:true,hardware:{gpu_percent:null}}),'UNAVAILABLE');
 assert.equal(screenValue(gpu,{available:true,hardware:{gpu_percent:13}}),'13%');
 assert.equal(screenValue(budget,{available:true,totals:{provider_cost_usd:0}}),'$0.0000');
 assert.equal(screenValue(budget,{available:true,totals:{provider_cost_usd:null}}),'UNAVAILABLE');
 assert.ok(dashboardDetail(budget,{available:true,totals:{provider_cost_usd:null}}).facts.includes('Recorded provider cost: UNAVAILABLE'));
 assert.ok(dashboardDetail(budget,{available:false,totals:{provider_cost_usd:0}}).facts.includes('Recorded provider cost: UNAVAILABLE'));
 const detail=dashboardDetail(gpu,{available:true,hardware:{gpu_percent:13,source:'OS sensor'},totals:{input_tokens:25,output_tokens:12},research:{phase:'watching'}});
 assert.ok(detail.facts.includes('Local tokens: 25 input / 12 output'));assert.ok(detail.facts.includes('OS sensor'));
});
