import test from 'node:test';
import assert from 'node:assert/strict';
import {roomStations,roomActivity,traverse,projectRoomPoint} from '../web/assets/room-activity.js';
test('every workroom has grounded chair/object anchors and each agent walks, interacts and sits',()=>{
 for(const [id,stations]of Object.entries(roomStations)){
  const seen=stations.map(()=>new Set());
  for(let time=0;time<stations.length*40000;time+=100){
   const poses=stations.map((_,seat)=>roomActivity(Number(id),seat,time,{occupants:stations.length}));
   assert.ok(poses.filter(p=>p.state==='walking'||p.state==='interacting').length<=1,'one journey per room');
   poses.forEach((pose,i)=>{seen[i].add(pose.state);assert.ok(pose.x>=180&&pose.x<=1420&&pose.y>=400&&pose.y<=915);assert.ok(pose.h>=75&&pose.h<=195);assert.ok(pose.object);});
  }
  for(const states of seen)assert.deepEqual([...states].sort(),['interacting','seated','walking']);
 }
});
test('standing preserves seated face scale and the CFO reaches the actual floor beside the cabinet',()=>{
 const chair=roomStations[9][0].chair,standing=roomActivity(9,0,0),interaction=roomActivity(9,0,10000);
 assert.ok(Math.abs(standing.y-standing.h-(chair.y-chair.h))<1,'head does not jump when rising');
 assert.equal(interaction.y,700);assert.ok(interaction.h>=chair.h/.73,'standing body is not a miniature portrait');
});
test('reduced motion stays seated and path traversal has exact endpoints',()=>{
 assert.equal(roomActivity(10,0,12000,{reducedMotion:true}).state,'seated');
 assert.deepEqual(traverse([{x:0,y:0},{x:3,y:0},{x:3,y:4}],0),{x:0,y:0});
 assert.deepEqual(traverse([{x:0,y:0},{x:3,y:0},{x:3,y:4}],1),{x:3,y:4});
 assert.deepEqual(traverse([{x:0,y:0},{x:3,y:0},{x:3,y:4}],.5),{x:3,y:.5});
});
test('exterior projection preserves the same room positions and proportions',()=>{
 const r={x:260,y:1000,w:900,h:460},a=projectRoomPoint(r,{x:400,y:700,h:110}),b=projectRoomPoint(r,{x:800,y:700,h:110});
 assert.ok(b.x>a.x);assert.equal(a.y,b.y);assert.equal(a.h,b.h);assert.ok(a.y>r.y&&a.y<r.y+r.h);
});
